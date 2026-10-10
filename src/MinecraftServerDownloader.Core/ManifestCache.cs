using System.Text.Json;

namespace MinecraftServerDownloader.Core;

public sealed class ManifestCache
{
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web);
    private readonly string _path;
    private readonly TimeSpan _duration;

    public ManifestCache(string path, TimeSpan? duration = null)
    {
        _path = path;
        _duration = duration ?? TimeSpan.FromMinutes(30);
    }

    public JsonElement? Read(string source)
    {
        try
        {
            if (!File.Exists(_path)) return null;
            CacheEntry? entry = JsonSerializer.Deserialize<CacheEntry>(File.ReadAllText(_path), JsonOptions);
            if (entry is null || entry.Source != source || DateTimeOffset.UtcNow - entry.Timestamp > _duration) return null;
            return entry.Manifest.Clone();
        }
        catch { return null; }
    }

    public void Write(string source, JsonElement manifest)
    {
        try
        {
            string directory = Path.GetDirectoryName(_path) ?? ".";
            Directory.CreateDirectory(directory);
            string temp = _path + ".tmp";
            File.WriteAllText(temp, JsonSerializer.Serialize(new CacheEntry { Source = source, Timestamp = DateTimeOffset.UtcNow, Manifest = manifest.Clone() }, JsonOptions));
            File.Move(temp, _path, true);
        }
        catch { }
    }

    private sealed class CacheEntry
    {
        public string Source { get; set; } = "";
        public DateTimeOffset Timestamp { get; set; }
        public JsonElement Manifest { get; set; }
    }
}

