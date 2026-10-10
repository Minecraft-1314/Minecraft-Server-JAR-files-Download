using System.IO;
using System.Text.Json;
using MinecraftServerDownloader.Core;

namespace MinecraftServerDownloader.Ui.Bridge;

public sealed class CommandRouter(Action<object> postEvent) : IDisposable
{
    private static readonly TimeSpan CacheDuration = TimeSpan.FromMinutes(30);
    private readonly OperationGate _operations = new();
    private readonly object _sync = new();
    private List<VersionInfo> _versions = [];
    private DownloadEngine? _engine;
    private bool _disposed;

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        _operations.Dispose();
    }

    public async Task<object> ExecuteAsync(string action, JsonElement payload)
    {
        return action switch
        {
            "getState" => BuildState(),
            "refreshVersions" => await RefreshVersionsAsync(payload).ConfigureAwait(true),
            "saveSettings" => SaveSettings(payload),
            "selectDirectory" => SelectDirectory(),
            "importSource" => ImportSource(payload),
            "removeSource" => RemoveSource(payload),
            "selectSource" => SelectSource(payload),
            "startDownload" => await StartDownloadAsync(payload).ConfigureAwait(true),
            "pauseDownload" => SetPaused(true),
            "resumeDownload" => SetPaused(false),
            "stopDownload" => StopDownload(),
            "cancel" => Cancel(),
            "openDataDirectory" => OpenDataDirectory(),
            _ => throw new AppValidationException("bridge.unknownAction")
        };
    }

    private object BuildState()
    {
        lock (_sync) return StateView.Build(ConfigStore.Settings(), ConfigStore.Sources(), _versions, ConfigStore.Credentials());
    }

    private async Task<object> RefreshVersionsAsync(JsonElement payload)
    {
        AppSettings settings = ReadSettings(payload);
        List<SourceProfile> sources = ConfigStore.Sources();
        using OperationGate.OperationLease lease = _operations.Create();
        try
        {
            postEvent(new { type = "status", key = "status.refreshing" });
            using SafeHttp http = CreateHttp(settings, sources);
            VersionCatalog catalog = new(http, new ManifestCache(Path.Combine(ConfigStore.DataDirectory, "manifest-cache.json"), CacheDuration), settings.DownloadDirectory);
            List<VersionInfo> versions = await catalog.LoadAsync(settings, sources, lease.Token).ConfigureAwait(true);
            lock (_sync) _versions = versions;
            return BuildState();
        }
        finally { postEvent(new { type = "status", key = "status.ready" }); }
    }

    private object SaveSettings(JsonElement payload)
    {
        ConfigStore.SaveSettings(ReadSettings(payload));
        return BuildState();
    }

    private static string SelectDirectory()
    {
        Microsoft.Win32.OpenFolderDialog dialog = new() { Title = "选择下载目录", InitialDirectory = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile) };
        return dialog.ShowDialog() == true ? dialog.FolderName : "";
    }

    private object ImportSource(JsonElement payload)
    {
        string address = UrlGuard.ValidatePublicUrl(BridgeJson.Text(payload, "address")).ToString();
        string id = Guid.NewGuid().ToString("N");
        SourceProfile source = new()
        {
            Id = id,
            Name = InputValidator.NormalizeName(BridgeJson.Text(payload, "name"), new Uri(address).Host),
            Address = address,
            Type = "manifest",
            CredentialType = "none",
            UserId = null
        };
        List<SourceProfile> sources = ConfigStore.Sources();
        if (sources.Any(item => item.Address.Equals(address, StringComparison.OrdinalIgnoreCase))) throw new AppValidationException("source.duplicate");
        sources.Add(source);
        ConfigStore.Save(new AppConfiguration { Settings = ConfigStore.Settings(), Sources = sources }, ConfigStore.Credentials());
        return BuildState();
    }

    private object RemoveSource(JsonElement payload)
    {
        string id = BridgeJson.Text(payload, "sourceId");
        List<SourceProfile> sources = ConfigStore.Sources();
        sources.RemoveAll(source => source.Id == id);
        Dictionary<string, string> credentials = ConfigStore.Credentials();
        credentials.Remove(id);
        AppSettings settings = ConfigStore.Settings();
        if (settings.CustomSourceId == id)
        {
            settings.CustomSourceId = null;
            settings.Mirror = "official";
        }
        ConfigStore.Save(new AppConfiguration { Settings = settings, Sources = sources }, credentials);
        return BuildState();
    }

    private object SelectSource(JsonElement payload)
    {
        string id = BridgeJson.Text(payload, "sourceId");
        AppSettings settings = ConfigStore.Settings();
        settings.Mirror = "custom";
        settings.CustomSourceId = ConfigStore.Sources().Any(source => source.Id == id) ? id : null;
        if (settings.CustomSourceId is null) settings.Mirror = "official";
        return StateView.Build(settings, ConfigStore.Sources(), CurrentVersions(), ConfigStore.Credentials());
    }

    private async Task<object> StartDownloadAsync(JsonElement payload)
    {
        AppSettings settings = ReadSettings(payload);
        string[] selected = payload.TryGetProperty("selectedIds", out JsonElement ids) && ids.ValueKind == JsonValueKind.Array
            ? ids.EnumerateArray().Select(item => item.GetString() ?? "").Where(item => item.Length > 0).ToArray()
            : [];
        List<VersionInfo> versions;
        lock (_sync) versions = _versions.Where(item => selected.Contains(item.Id, StringComparer.Ordinal)).ToList();
        if (versions.Count == 0) throw new AppValidationException("download.noSelection");
        Directory.CreateDirectory(InputValidator.NormalizeDirectory(settings.DownloadDirectory));
        List<SourceProfile> sources = ConfigStore.Sources();
        using OperationGate.OperationLease lease = _operations.Create();
        using SafeHttp http = CreateHttp(settings, sources);
        VersionCatalog catalog = new(http, new ManifestCache(Path.Combine(ConfigStore.DataDirectory, "manifest-cache.json"), CacheDuration), settings.DownloadDirectory);
        DownloadEngine engine = new(http, catalog);
        lock (_sync) _engine = engine;
        try
        {
            DownloadSummary summary = await engine.RunAsync(versions, settings, sources, new Progress<DownloadProgress>(value => postEvent(new { type = "progress", value })), (key, error) => postEvent(new { type = "log", key, error }), lease.Token).ConfigureAwait(true);
            lock (_sync) _versions = _versions.Select(item => versions.Any(version => version.Id == item.Id) ? item with { IsDownloaded = summary.Outcome == DownloadOutcome.Completed } : item).ToList();
            return summary;
        }
        finally { lock (_sync) _engine = null; }
    }

    private bool SetPaused(bool paused)
    {
        DownloadEngine? engine;
        lock (_sync) engine = _engine;
        if (engine is null) throw new AppValidationException("download.notRunning");
        if (paused) engine.Pause(); else engine.Resume();
        return true;
    }

    private bool StopDownload()
    {
        DownloadEngine? engine;
        lock (_sync) engine = _engine;
        if (engine is null) throw new AppValidationException("download.notRunning");
        engine.Stop();
        return true;
    }

    private bool Cancel()
    {
        DownloadEngine? engine;
        lock (_sync) engine = _engine;
        engine?.Stop();
        _operations.CancelCurrent();
        return true;
    }

    private static bool OpenDataDirectory()
    {
        ExternalLauncher.OpenDataDirectory();
        return true;
    }

    private static SafeHttp CreateHttp(AppSettings settings, IReadOnlyList<SourceProfile> sources)
    {
        SourceProfile? source = settings.Mirror == "custom" ? sources.FirstOrDefault(item => item.Id == settings.CustomSourceId) : null;
        if (settings.Mirror == "custom" && source is null) throw new AppValidationException("source.missing");
        Dictionary<string, string> credentials = ConfigStore.Credentials();
        string credential = source is null || !credentials.TryGetValue(source.Id, out string? value) ? "" : value;
        return new SafeHttp(settings, source?.CredentialType ?? "none", credential, source?.UserId);
    }

    private List<VersionInfo> CurrentVersions()
    {
        lock (_sync) return _versions.ToList();
    }

    private static AppSettings ReadSettings(JsonElement payload)
    {
        JsonElement node = BridgeJson.Object(payload, "settings");
        if (node.ValueKind != JsonValueKind.Object) node = payload;
        AppSettings current = ConfigStore.Settings();
        return new AppSettings
        {
            DownloadDirectory = BridgeJson.Text(node, "downloadDirectory", current.DownloadDirectory),
            Mirror = BridgeJson.Text(node, "mirror", current.Mirror),
            CustomSourceId = BridgeJson.Text(node, "customSourceId", current.CustomSourceId ?? ""),
            Proxy = BridgeJson.Text(node, "proxy", current.Proxy),
            MaxConcurrent = InputValidator.NormalizeInteger(BridgeJson.Int(node, "maxConcurrent", current.MaxConcurrent), 1, InputValidator.MaxConcurrentDownloads, 3),
            RequestTimeoutSeconds = InputValidator.NormalizeInteger(BridgeJson.Int(node, "requestTimeoutSeconds", current.RequestTimeoutSeconds), InputValidator.MinTimeoutSeconds, InputValidator.MaxTimeoutSeconds, 30),
            RetryCount = InputValidator.NormalizeInteger(BridgeJson.Int(node, "retryCount", current.RetryCount), 0, InputValidator.MaxRetryCount, 2),
            Language = BridgeJson.Text(node, "language", current.Language),
            Theme = BridgeJson.Text(node, "theme", current.Theme)
        };
    }
}
