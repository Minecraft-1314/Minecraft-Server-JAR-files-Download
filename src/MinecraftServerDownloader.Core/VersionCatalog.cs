using System.Text.Json;

namespace MinecraftServerDownloader.Core;

public sealed class VersionCatalog
{
    private static readonly string[] OfficialManifests = ["https://launchermeta.mojang.com/mc/game/version_manifest.json", "https://piston-meta.mojang.com/mc/game/version_manifest.json"];
    private static readonly string BmclManifest = "https://bmclapi2.bangbang93.com/mc/game/version_manifest.json";
    private const string BmclMetadata = "https://bmclapi2.bangbang93.com/version/{0}/json";
    private const string BmclServer = "https://bmclapi2.bangbang93.com/version/{0}/server";

    private readonly SafeHttp _http;
    private readonly ManifestCache _cache;
    private readonly string _downloadDirectory;

    public VersionCatalog(SafeHttp http, ManifestCache cache, string downloadDirectory)
    {
        _http = http;
        _cache = cache;
        _downloadDirectory = InputValidator.NormalizeDirectory(downloadDirectory);
    }

    public async Task<List<VersionInfo>> LoadAsync(AppSettings settings, IReadOnlyList<SourceProfile> sources, CancellationToken cancellationToken)
    {
        Uri[] urls = ManifestUrls(settings, sources);
        string sourceKey = SourceKey(settings, urls);
        JsonElement manifest = _cache.Read(sourceKey) ?? await FetchManifestAsync(urls, sourceKey, cancellationToken).ConfigureAwait(false);
        return ParseManifest(manifest, _downloadDirectory);
    }

    public async Task<VersionJob?> ResolveAsync(AppSettings settings, IReadOnlyList<SourceProfile> sources, VersionInfo version, CancellationToken cancellationToken)
    {
        Uri metadataUrl = settings.Mirror == "bmclapi" ? new Uri(string.Format(System.Globalization.CultureInfo.InvariantCulture, BmclMetadata, version.Id)) : new Uri(version.Url);
        JsonElement metadata = await _http.GetJsonAsync(metadataUrl, cancellationToken).ConfigureAwait(false);
        if (!metadata.TryGetProperty("downloads", out JsonElement downloads)
            || !downloads.TryGetProperty("server", out JsonElement server)
            || !server.TryGetProperty("url", out JsonElement urlElement))
            return null;
        string serverUrl = settings.Mirror == "bmclapi" ? string.Format(System.Globalization.CultureInfo.InvariantCulture, BmclServer, version.Id) : urlElement.GetString() ?? "";
        if (serverUrl.Length == 0) return null;
        Uri validated = UrlGuard.ValidatePublicUrl(serverUrl);
        long size = server.TryGetProperty("size", out JsonElement sizeElement) && sizeElement.TryGetInt64(out long parsedSize) ? parsedSize : 0;
        string sha1 = server.TryGetProperty("sha1", out JsonElement sha1Element) ? sha1Element.GetString() ?? "" : "";
        return new VersionJob(version, validated.ToString(), sha1, size);
    }

    private async Task<JsonElement> FetchManifestAsync(Uri[] urls, string sourceKey, CancellationToken cancellationToken)
    {
        Exception? last = null;
        foreach (Uri url in urls)
        {
            try
            {
                JsonElement manifest = await _http.GetJsonAsync(url, cancellationToken).ConfigureAwait(false);
                _cache.Write(sourceKey, manifest);
                return manifest;
            }
            catch (Exception error) when (error is not OperationCanceledException) { last = error; }
        }
        throw new AppException(last is null ? "manifest.failed" : "manifest.failed: " + last.Message);
    }

    public static List<VersionInfo> ParseManifest(JsonElement manifest, string downloadDirectory)
    {
        if (manifest.ValueKind != JsonValueKind.Object || !manifest.TryGetProperty("versions", out JsonElement versions) || versions.ValueKind != JsonValueKind.Array) throw new AppValidationException("manifest.invalid");
        string root = InputValidator.NormalizeDirectory(downloadDirectory);
        List<VersionInfo> result = [];
        foreach (JsonElement element in versions.EnumerateArray())
        {
            if (element.ValueKind != JsonValueKind.Object || !element.TryGetProperty("type", out JsonElement typeElement) || !string.Equals(typeElement.GetString(), "release", StringComparison.OrdinalIgnoreCase) || !element.TryGetProperty("id", out JsonElement idElement)) continue;
            string id = idElement.GetString() ?? "";
            if (!Uri.TryCreate(element.TryGetProperty("url", out JsonElement urlElement) ? urlElement.GetString() ?? "" : "", UriKind.Absolute, out Uri? url) || !UrlGuard.IsPublicUri(url)) continue;
            try { id = InputValidator.NormalizeVersionId(id); } catch { continue; }
            DateTimeOffset release = element.TryGetProperty("releaseTime", out JsonElement releaseElement) && DateTimeOffset.TryParse(releaseElement.GetString(), out DateTimeOffset parsed) ? parsed : DateTimeOffset.MinValue;
            string safe = InputValidator.SanitizeFileName(id);
            string folder = FolderName(id);
            result.Add(new VersionInfo(id, url.ToString(), $"minecraft_server_{safe}.jar", folder, release, File.Exists(Path.Combine(root, folder, $"minecraft_server_{safe}.jar"))));
        }
        return result.OrderByDescending(item => item.ReleaseTime).ThenBy(item => item.Id, StringComparer.OrdinalIgnoreCase).ToList();
    }

    public static string FolderName(string version)
    {
        string[] parts = version.Split('.');
        return parts.Length >= 2 ? $"Minecraft {parts[0]}.{parts[1]} Server JAR files" : $"Minecraft {version} Server JAR files";
    }

    private static Uri[] ManifestUrls(AppSettings settings, IReadOnlyList<SourceProfile> sources)
    {
        if (settings.Mirror == "bmclapi") return [new Uri(BmclManifest)];
        if (settings.Mirror == "custom")
        {
            SourceProfile? source = sources.FirstOrDefault(item => item.Id == settings.CustomSourceId);
            if (source is null) throw new AppValidationException("source.missing");
            return [UrlGuard.ValidatePublicUrl(source.Address)];
        }
        return OfficialManifests.Select(url => new Uri(url)).ToArray();
    }

    private static string SourceKey(AppSettings settings, Uri[] urls)
        => settings.Mirror == "custom" ? "custom:" + string.Join("|", urls.Select(url => url.ToString())) : settings.Mirror;
}


