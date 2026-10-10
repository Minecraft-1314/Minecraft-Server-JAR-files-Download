using System.IO;
using MinecraftServerDownloader.Core;

namespace MinecraftServerDownloader.Ui.Bridge;

public static class ExternalLauncher
{
    public static void Open(string value)
    {
        if (!UrlGuard.IsPublicUrl(value)) throw new AppValidationException("url.blocked");
        System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(value) { UseShellExecute = true });
    }

    public static void OpenDataDirectory()
    {
        Directory.CreateDirectory(ConfigStore.DataDirectory);
        System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo { FileName = ConfigStore.DataDirectory, UseShellExecute = true });
    }
}

