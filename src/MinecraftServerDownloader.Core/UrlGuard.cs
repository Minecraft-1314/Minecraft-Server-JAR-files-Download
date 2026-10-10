using System.Net;
using System.Net.Sockets;

namespace MinecraftServerDownloader.Core;

public static class UrlGuard
{
    private static readonly string[] BlockedHostSuffixes = [".local", ".internal", ".home", ".lan", "localhost", "metadata.google.internal"];

    public static Uri ValidatePublicUrl(string value)
    {
        if (!Uri.TryCreate(InputValidator.Clean(value), UriKind.Absolute, out Uri? uri)) throw new AppValidationException("url.invalid");
        if (!IsPublicUri(uri)) throw new AppValidationException("url.blocked");
        return uri;
    }

    public static bool IsPublicUrl(string value)
    {
        try { return Uri.TryCreate(value, UriKind.Absolute, out Uri? uri) && IsPublicUri(uri); } catch { return false; }
    }

    public static bool IsPublicUri(Uri uri)
    {
        if (uri.Scheme is not ("http" or "https") || uri.UserInfo.Length > 0 || string.IsNullOrWhiteSpace(uri.IdnHost) || BlockedHostSuffixes.Any(item => uri.IdnHost.EndsWith(item, StringComparison.OrdinalIgnoreCase))) return false;
        return !IsBlockedLiteral(uri.IdnHost);
    }

    public static async Task<IPAddress> ResolvePublicAddressAsync(string host, CancellationToken cancellationToken)
    {
        string clean = InputValidator.Clean(host);
        if (IPAddress.TryParse(clean, out IPAddress? literal))
        {
            if (!IsAllowedAddress(literal)) throw new AppValidationException("url.blocked");
            return literal;
        }
        IPAddress[] addresses = await Dns.GetHostAddressesAsync(clean, cancellationToken).ConfigureAwait(false);
        return addresses.FirstOrDefault(IsAllowedAddress) ?? throw new AppValidationException("url.blocked");
    }

    public static void ValidatePublicUri(Uri uri) => _ = IsPublicUri(uri);

    private static bool IsBlockedLiteral(string host) => IPAddress.TryParse(host, out IPAddress? address) && !IsAllowedAddress(address);

    private static bool IsAllowedAddress(IPAddress address)
    {
        if (address.AddressFamily is not (AddressFamily.InterNetwork or AddressFamily.InterNetworkV6)) return false;
        if (address.IsIPv4MappedToIPv6) address = address.MapToIPv4();
        if (IPAddress.IsLoopback(address) || address.Equals(IPAddress.Any) || address.Equals(IPAddress.IPv6Any) || address.Equals(IPAddress.Broadcast) || address.IsIPv6LinkLocal || address.IsIPv6Multicast || address.IsIPv6SiteLocal || address.IsIPv6UniqueLocal) return false;
        return address.AddressFamily == AddressFamily.InterNetwork ? IsPublicIpv4(address.GetAddressBytes()) : IsPublicIpv6(address.GetAddressBytes());
    }

    private static bool IsPublicIpv4(byte[] bytes) => bytes[0] switch
    {
        0 => false,
        10 => false,
        100 => bytes[1] < 64 || bytes[1] > 127,
        127 => false,
        169 => bytes[1] != 254,
        172 => bytes[1] < 16 || bytes[1] > 31,
        192 => bytes[1] switch { 0 => false, 168 => false, 18 when bytes[2] >= 18 && bytes[2] <= 19 => false, _ => true },
        198 => bytes[1] is < 18 or > 31,
        224 => false,
        255 => false,
        _ => true
    };

    private static bool IsPublicIpv6(byte[] bytes)
    {
        if ((bytes[0] & 0xFE) == 0xFC) return false;
        if (bytes[0] == 0x20 && bytes[1] == 0x01 && bytes[2] == 0x0D && bytes[3] == 0xB8) return false;
        if (bytes[0] == 0x00 && bytes.Skip(1).All(item => item == 0)) return false;
        if (bytes[0] == 0x64 && bytes[1] == 0xFF && bytes[2] == 0x9B)
        {
            byte[] mapped = bytes.Skip(12).Take(4).ToArray();
            return mapped.Length == 4 && IsPublicIpv4(mapped);
        }
        if (bytes[0] == 0x20 && bytes[1] == 0x02)
        {
            byte[] mapped = bytes.Skip(2).Take(4).ToArray();
            return mapped.Length == 4 && IsPublicIpv4(mapped);
        }
        if (bytes[0] == 0x20 && bytes[1] == 0x01 && bytes[2] == 0x00 && (bytes[3] & 0xC0) == 0) return false;
        return true;
    }
}

