namespace MinecraftServerDownloader.Core;

public static class DiskSpace
{
    public static long AvailableBytes(string path)
    {
        string probe = InputValidator.NormalizeDirectory(path);
        while (!Directory.Exists(probe))
        {
            string? parent = Path.GetDirectoryName(probe);
            if (parent is null || parent == probe) break;
            probe = parent;
        }
        try { return new DriveInfo(Path.GetPathRoot(probe) ?? probe).AvailableFreeSpace; }
        catch { return 0; }
    }
}
