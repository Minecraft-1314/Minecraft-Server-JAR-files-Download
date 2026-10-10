using System.Text.RegularExpressions;

namespace MinecraftServerDownloader.Core;

public static partial class InputValidator
{
    public const int MaxNameLength = 80;
    public const int MaxAddressLength = 2048;
    public const int MaxCredentialLength = 4096;
    public const int MaxConcurrentDownloads = 8;
    public const int MinTimeoutSeconds = 5;
    public const int MaxTimeoutSeconds = 300;
    public const int MaxRetryCount = 5;

    [GeneratedRegex(@"^[A-Za-z0-9._-]{1,64}$", RegexOptions.CultureInvariant)]
    private static partial Regex VersionIdPattern();

    public static string NormalizeName(string? value, string fallback = "")
    {
        string text = Clean(value);
        if (text.Length == 0) text = Clean(fallback);
        if (text.Length == 0) throw new AppValidationException("name.required");
        if (text.Length > MaxNameLength) throw new AppValidationException("name.tooLong");
        return text;
    }

    public static string NormalizeVersionId(string? value)
    {
        string text = Clean(value);
        if (!VersionIdPattern().IsMatch(text)) throw new AppValidationException("version.invalid");
        return text;
    }

    public static int NormalizeInteger(int value, int minimum, int maximum, int fallback)
        => value < minimum || value > maximum ? fallback : value;

    public static long? NormalizeUserId(object? value)
    {
        if (value is null || value is string emptyText && Clean(emptyText).Length == 0) return null;
        long parsed = value switch
        {
            long number => number,
            int number => number,
            string text when long.TryParse(text.Trim(), out long number) => number,
            _ => 0
        };
        return parsed is >= 1 and <= 999_999_999_999 ? parsed : null;
    }

    public static string NormalizeCredential(string? value)
    {
        string text = Clean(value);
        if (text.Length > MaxCredentialLength) throw new AppValidationException("credential.tooLong");
        return text;
    }

    public static string NormalizeDirectory(string? value)
    {
        string text = Clean(value);
        if (text.Length == 0) throw new AppValidationException("directory.required");
        try
        {
            string full = Path.GetFullPath(text);
            if (full.IndexOfAny(Path.GetInvalidPathChars()) >= 0) throw new AppValidationException("directory.invalid");
            return full.TrimEnd(Path.DirectorySeparatorChar);
        }
        catch (Exception error) when (error is ArgumentException or NotSupportedException or PathTooLongException)
        {
            throw new AppValidationException("directory.invalid");
        }
    }

    public static string SanitizeFileName(string value)
    {
        string cleaned = Clean(value);
        foreach (char invalid in Path.GetInvalidFileNameChars()) cleaned = cleaned.Replace(invalid.ToString(), "");
        return cleaned.Replace(" ", "");
    }

    public static string Clean(string? value) => (value ?? "").Trim();
}

