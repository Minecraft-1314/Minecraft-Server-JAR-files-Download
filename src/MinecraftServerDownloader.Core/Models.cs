namespace MinecraftServerDownloader.Core;

public enum MirrorKind
{
    Official,
    Bmclapi,
    Custom
}

public enum DownloadOutcome
{
    Completed,
    Stopped,
    DiskFull,
    Failed
}

public sealed class AppSettings
{
    public string DownloadDirectory { get; set; } = "";
    public string Mirror { get; set; } = "official";
    public string? CustomSourceId { get; set; }
    public string Proxy { get; set; } = "";
    public int MaxConcurrent { get; set; } = 3;
    public int RequestTimeoutSeconds { get; set; } = 30;
    public int RetryCount { get; set; } = 2;
    public string Language { get; set; } = "zh-cn";
    public string Theme { get; set; } = "system";
}

public sealed class SourceProfile
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public string Address { get; set; } = "";
    public string Type { get; set; } = "manifest";
    public string CredentialType { get; set; } = "none";
    public long? UserId { get; set; }
}

public sealed class AppConfiguration
{
    public int Version { get; set; } = 1;
    public AppSettings Settings { get; set; } = new();
    public List<SourceProfile> Sources { get; set; } = [];
}

public sealed record VersionInfo(string Id, string Url, string JarName, string Folder, DateTimeOffset ReleaseTime, bool IsDownloaded);

public sealed record VersionJob(VersionInfo Version, string ServerUrl, string Sha1, long Size);

public sealed record DownloadProgress(int Completed, int Total, long DoneBytes, long TotalBytes, double BytesPerSecond, double RemainingSeconds);

public sealed record DownloadSummary(DownloadOutcome Outcome, int Completed, int Failed, long TotalBytes);

public class AppException(string message) : Exception(message);

public sealed class AppValidationException(string message) : AppException(message);

