using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace MinecraftServerDownloader.Core;

public static class ConfigStore
{
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web) { WriteIndented = true, AllowTrailingCommas = false };
    private static readonly object Sync = new();
    private static AppConfiguration _configuration = new();
    private static string? _lastError;

    public static string DataDirectory { get; } = Environment.GetEnvironmentVariable("MSD_DATA_DIR") is { Length: > 0 } value
        ? Path.GetFullPath(value)
        : Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "MinecraftServerDownloader");

    public static string ConfigPath => Path.Combine(DataDirectory, "config.json");
    public static string CredentialPath => Path.Combine(DataDirectory, "credentials.bin");
    public static bool ConfigExists => File.Exists(ConfigPath);
    public static string? LastError { get { lock (Sync) return _lastError; } }

    public static AppConfiguration Load()
    {
        lock (Sync)
        {
            _lastError = null;
            _configuration = Normalize(new AppConfiguration());
            if (!File.Exists(ConfigPath)) return Clone(_configuration);
            try
            {
                AppConfiguration? loaded = JsonSerializer.Deserialize<AppConfiguration>(File.ReadAllText(ConfigPath), JsonOptions);
                _configuration = Normalize(loaded ?? new AppConfiguration());
            }
            catch (Exception error) when (error is JsonException or IOException or UnauthorizedAccessException)
            {
                _lastError = "config.ignored";
            }
            return Clone(_configuration);
        }
    }

    public static void Save(AppConfiguration configuration, IReadOnlyDictionary<string, string> credentials)
    {
        AppConfiguration normalized = Normalize(configuration);
        Dictionary<string, string> clean = credentials
            .Where(pair => !string.IsNullOrWhiteSpace(pair.Value))
            .ToDictionary(pair => pair.Key, pair => InputValidator.NormalizeCredential(pair.Value), StringComparer.Ordinal);
        lock (Sync)
        {
            Directory.CreateDirectory(DataDirectory);
            WriteJson(ConfigPath, normalized);
            WriteProtected(CredentialPath, clean);
            _configuration = normalized;
            _lastError = null;
        }
    }

    public static void SaveSettings(AppSettings settings)
    {
        lock (Sync)
        {
            _configuration.Settings = NormalizeSettings(settings);
            Directory.CreateDirectory(DataDirectory);
            WriteJson(ConfigPath, _configuration);
        }
    }

    public static AppSettings Settings() { lock (Sync) return Clone(_configuration).Settings; }

    public static List<SourceProfile> Sources() { lock (Sync) return Clone(_configuration).Sources; }

    public static Dictionary<string, string> Credentials()
    {
        lock (Sync)
        {
            if (!File.Exists(CredentialPath)) return new Dictionary<string, string>(StringComparer.Ordinal);
            try { return ReadProtected(CredentialPath); }
            catch { _lastError = "credentials.ignored"; return new Dictionary<string, string>(StringComparer.Ordinal); }
        }
    }

    public static void ResetForTests()
    {
        lock (Sync)
        {
            _configuration = new AppConfiguration();
            _lastError = null;
            if (Directory.Exists(DataDirectory)) Directory.Delete(DataDirectory, true);
        }
    }

    private static AppConfiguration Normalize(AppConfiguration configuration)
    {
        configuration.Version = 1;
        configuration.Settings = NormalizeSettings(configuration.Settings ?? new AppSettings());
        configuration.Sources ??= [];
        configuration.Sources = configuration.Sources
            .Where(source => !string.IsNullOrWhiteSpace(source?.Address))
            .Select(source =>
            {
                source.Id = InputValidator.SanitizeFileName(InputValidator.Clean(source.Id));
                if (source.Id.Length == 0) source.Id = Guid.NewGuid().ToString("N");
                source.Name = InputValidator.NormalizeName(source.Name, source.Id);
                source.Address = UrlGuard.ValidatePublicUrl(source.Address).ToString();
                source.Type = source.Type.Equals("bmclapi", StringComparison.OrdinalIgnoreCase) ? "bmclapi" : "manifest";
                source.CredentialType = source.CredentialType.Equals("bearer", StringComparison.OrdinalIgnoreCase) ? "bearer" : "none";
                source.UserId = InputValidator.NormalizeUserId(source.UserId);
                return source;
            })
            .GroupBy(source => source.Id, StringComparer.Ordinal)
            .Select(group => group.First())
            .ToList();
        if (configuration.Settings.CustomSourceId is { Length: > 0 } id && !configuration.Sources.Any(source => source.Id == id)) configuration.Settings.CustomSourceId = null;
        return configuration;
    }

    private static AppSettings NormalizeSettings(AppSettings settings)
    {
        settings.Mirror = settings.Mirror.ToLowerInvariant() switch { "bmclapi" => "bmclapi", "custom" => "custom", _ => "official" };
        settings.MaxConcurrent = InputValidator.NormalizeInteger(settings.MaxConcurrent, 1, InputValidator.MaxConcurrentDownloads, 3);
        settings.RequestTimeoutSeconds = InputValidator.NormalizeInteger(settings.RequestTimeoutSeconds, InputValidator.MinTimeoutSeconds, InputValidator.MaxTimeoutSeconds, 30);
        settings.RetryCount = InputValidator.NormalizeInteger(settings.RetryCount, 0, InputValidator.MaxRetryCount, 2);
        settings.Language = settings.Language.StartsWith("zh", StringComparison.OrdinalIgnoreCase) ? "zh-cn" : "en-us";
        settings.Theme = settings.Theme.ToLowerInvariant() switch { "light" => "light", "dark" => "dark", _ => "system" };
        settings.Proxy = InputValidator.Clean(settings.Proxy);
        if (settings.Proxy.Length > 0) settings.Proxy = UrlGuard.ValidatePublicUrl(settings.Proxy).ToString();
        settings.DownloadDirectory = string.IsNullOrWhiteSpace(settings.DownloadDirectory)
            ? Environment.GetFolderPath(Environment.SpecialFolder.UserProfile)
            : InputValidator.NormalizeDirectory(settings.DownloadDirectory);
        return settings;
    }

    private static void WriteJson(string path, AppConfiguration configuration)
    {
        string temp = path + ".tmp";
        File.WriteAllText(temp, JsonSerializer.Serialize(configuration, JsonOptions), Encoding.UTF8);
        File.Move(temp, path, true);
    }

    private static void WriteProtected(string path, IReadOnlyDictionary<string, string> credentials)
    {
        string json = JsonSerializer.Serialize(credentials, JsonOptions);
        byte[] bytes = ProtectedData.Protect(Encoding.UTF8.GetBytes(json), null, DataProtectionScope.CurrentUser);
        File.WriteAllBytes(path, bytes);
    }

    private static Dictionary<string, string> ReadProtected(string path)
    {
        byte[] bytes = ProtectedData.Unprotect(File.ReadAllBytes(path), null, DataProtectionScope.CurrentUser);
        return JsonSerializer.Deserialize<Dictionary<string, string>>(Encoding.UTF8.GetString(bytes), JsonOptions) ?? new Dictionary<string, string>(StringComparer.Ordinal);
    }

    private static AppConfiguration Clone(AppConfiguration source)
    {
        return new AppConfiguration
        {
            Version = source.Version,
            Settings = new AppSettings
            {
                DownloadDirectory = source.Settings.DownloadDirectory,
                Mirror = source.Settings.Mirror,
                CustomSourceId = source.Settings.CustomSourceId,
                Proxy = source.Settings.Proxy,
                MaxConcurrent = source.Settings.MaxConcurrent,
                RequestTimeoutSeconds = source.Settings.RequestTimeoutSeconds,
                RetryCount = source.Settings.RetryCount,
                Language = source.Settings.Language,
                Theme = source.Settings.Theme
            },
            Sources = source.Sources.Select(source => new SourceProfile
            {
                Id = source.Id,
                Name = source.Name,
                Address = source.Address,
                Type = source.Type,
                CredentialType = source.CredentialType,
                UserId = source.UserId
            }).ToList()
        };
    }
}


