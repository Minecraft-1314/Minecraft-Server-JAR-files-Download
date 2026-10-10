using System.Net;
using System.Net.Http.Headers;
using System.Net.Sockets;
using System.Text.Json;

namespace MinecraftServerDownloader.Core;

public sealed class SafeHttp : IDisposable
{
    private static readonly int[] RetryStatuses = [408, 429, 500, 502, 503, 504];
    private readonly HttpClient _client;
    private readonly int _timeoutSeconds;
    private readonly int _retryCount;
    private readonly string _credentialType;
    private readonly string _credential;
    private readonly long? _userId;

    public SafeHttp(AppSettings settings, string credentialType = "none", string credential = "", long? userId = null, HttpMessageHandler? handler = null)
    {
        _timeoutSeconds = InputValidator.NormalizeInteger(settings.RequestTimeoutSeconds, InputValidator.MinTimeoutSeconds, InputValidator.MaxTimeoutSeconds, 30);
        _retryCount = InputValidator.NormalizeInteger(settings.RetryCount, 0, InputValidator.MaxRetryCount, 2);
        _credentialType = InputValidator.Clean(credentialType);
        _credential = InputValidator.NormalizeCredential(credential);
        _userId = userId;
        handler ??= CreateHandler(settings);
        _client = new HttpClient(handler) { Timeout = Timeout.InfiniteTimeSpan };
    }

    public async Task<JsonElement> GetJsonAsync(Uri uri, CancellationToken cancellationToken)
    {
        using HttpResponseMessage response = await GetAsync(uri, false, 0, cancellationToken).ConfigureAwait(false);
        await using Stream stream = await response.Content.ReadAsStreamAsync(cancellationToken).ConfigureAwait(false);
        using JsonDocument document = await JsonDocument.ParseAsync(stream, cancellationToken: cancellationToken).ConfigureAwait(false);
        return document.RootElement.Clone();
    }

    public Task<HttpResponseMessage> GetDownloadAsync(Uri uri, long offset, CancellationToken cancellationToken) => GetAsync(uri, true, offset, cancellationToken);

    private static SocketsHttpHandler CreateHandler(AppSettings settings)
    {
        SocketsHttpHandler sockets = new()
        {
            AllowAutoRedirect = false,
            AutomaticDecompression = DecompressionMethods.None,
            ConnectTimeout = TimeSpan.FromSeconds(Math.Min(InputValidator.NormalizeInteger(settings.RequestTimeoutSeconds, InputValidator.MinTimeoutSeconds, InputValidator.MaxTimeoutSeconds, 30), 15)),
            MaxConnectionsPerServer = InputValidator.NormalizeInteger(settings.MaxConcurrent, 1, InputValidator.MaxConcurrentDownloads, 3),
            ConnectCallback = ConnectAsync
        };
        if (!string.IsNullOrWhiteSpace(settings.Proxy))
        {
            sockets.Proxy = new WebProxy(UrlGuard.ValidatePublicUrl(settings.Proxy));
            sockets.UseProxy = true;
        }
        return sockets;
    }

    private async Task<HttpResponseMessage> GetAsync(Uri uri, bool responseHeadersRead, long offset, CancellationToken cancellationToken)
    {
        Uri current = UrlGuard.ValidatePublicUrl(uri.ToString());
        string initialOrigin = current.GetLeftPart(UriPartial.Authority);
        for (int attempt = 0; ; attempt++)
        {
            using CancellationTokenSource attemptSource = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
            attemptSource.CancelAfter(TimeSpan.FromSeconds(_timeoutSeconds));
            try
            {
                using HttpRequestMessage request = CreateRequest(current, responseHeadersRead, offset, current.GetLeftPart(UriPartial.Authority) == initialOrigin);
                HttpResponseMessage response = await _client.SendAsync(request, responseHeadersRead ? HttpCompletionOption.ResponseHeadersRead : HttpCompletionOption.ResponseContentRead, attemptSource.Token).ConfigureAwait(false);
                if (IsRedirect(response.StatusCode))
                {
                    Uri? location = response.Headers.Location;
                    response.Dispose();
                    if (location is null) throw new AppException("network.redirect");
                    Uri next = location.IsAbsoluteUri ? location : new Uri(current, location);
                    current = UrlGuard.ValidatePublicUrl(next.ToString());
                    continue;
                }
                if (IsRetryStatus(response.StatusCode) && attempt < _retryCount)
                {
                    int delay = Math.Min(1000 * (1 << attempt), 8000);
                    response.Dispose();
                    await Task.Delay(delay, cancellationToken).ConfigureAwait(false);
                    continue;
                }
                if (!response.IsSuccessStatusCode)
                {
                    int status = (int)response.StatusCode;
                    response.Dispose();
                    throw new AppException($"http.{status}");
                }
                return response;
            }
            catch (Exception error) when (error is HttpRequestException or TaskCanceledException && !cancellationToken.IsCancellationRequested && attempt < _retryCount)
            {
                await Task.Delay(Math.Min(1000 * (1 << attempt), 8000), cancellationToken).ConfigureAwait(false);
            }
            catch (Exception error) when (error is not AppValidationException && error is not AppException && !cancellationToken.IsCancellationRequested && attempt >= _retryCount)
            {
                throw new AppException("network.failed");
            }
        }
    }

    private HttpRequestMessage CreateRequest(Uri uri, bool responseHeadersRead, long offset, bool includeCredentials)
    {
        HttpRequestMessage request = new(HttpMethod.Get, uri);
        request.Headers.Accept.Add(new MediaTypeWithQualityHeaderValue(responseHeadersRead ? "application/octet-stream" : "application/json"));
        request.Headers.UserAgent.ParseAdd("MinecraftServerDownloader/2.0");
        if (offset > 0) request.Headers.Range = new RangeHeaderValue(offset, null);
        if (includeCredentials && _credentialType.Equals("bearer", StringComparison.OrdinalIgnoreCase) && _credential.Length > 0) request.Headers.Authorization = new AuthenticationHeaderValue("Bearer", _credential);
        if (includeCredentials && _userId.HasValue) request.Headers.TryAddWithoutValidation("X-User-Id", _userId.Value.ToString(System.Globalization.CultureInfo.InvariantCulture));
        return request;
    }

    private static bool IsRetryStatus(HttpStatusCode status) => RetryStatuses.Contains((int)status);

    private static bool IsRedirect(HttpStatusCode status) => (int)status is 301 or 302 or 303 or 307 or 308;

    private static async ValueTask<Stream> ConnectAsync(SocketsHttpConnectionContext context, CancellationToken cancellationToken)
    {
        Uri uri = context.InitialRequestMessage.RequestUri ?? throw new AppValidationException("url.invalid");
        IPAddress address = await UrlGuard.ResolvePublicAddressAsync(uri.IdnHost, cancellationToken).ConfigureAwait(false);
        Socket socket = new(address.AddressFamily, SocketType.Stream, ProtocolType.Tcp) { NoDelay = true };
        try
        {
            await socket.ConnectAsync(address, uri.Port, cancellationToken).ConfigureAwait(false);
            return new NetworkStream(socket, ownsSocket: true);
        }
        catch
        {
            socket.Dispose();
            throw;
        }
    }

    public void Dispose() => _client.Dispose();
}

