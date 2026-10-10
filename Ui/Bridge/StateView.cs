using MinecraftServerDownloader.Core;

namespace MinecraftServerDownloader.Ui.Bridge;

public static class StateView
{
    public static object Build(AppSettings settings, IReadOnlyList<SourceProfile> sources, IReadOnlyList<VersionInfo> versions, IReadOnlyDictionary<string, string> credentials)
    {
        return new
        {
            settings,
            sources = sources.Select(source => new
            {
                source.Id,
                source.Name,
                source.Address,
                source.Type,
                source.CredentialType,
                source.UserId,
                hasCredential = credentials.ContainsKey(source.Id)
            }),
            versions,
            configExists = ConfigStore.ConfigExists,
            lastError = ConfigStore.LastError,
            dataDirectory = ConfigStore.DataDirectory
        };
    }
}
