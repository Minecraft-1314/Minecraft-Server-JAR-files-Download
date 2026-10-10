using System.Globalization;
using System.IO;
using System.Text.Json;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using MinecraftServerDownloader.Core;
using MinecraftServerDownloader.Ui.Bridge;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.Wpf;

namespace MinecraftServerDownloader.Ui;

public sealed class WebViewWindow : Window, IDisposable
{
    private const string VirtualHost = "app.local";
    private const string EntryPage = "https://app.local/index.html";
    private const int MaxMessageCharacters = 2_000_000;
    private readonly WebView2 _view = new();
    private CommandRouter? _router;
    private bool _disposed;

    public WebViewWindow()
    {
        Title = "Minecraft Server Downloader";
        Width = 1320;
        Height = 860;
        MinWidth = 360;
        MinHeight = 640;
        WindowStartupLocation = WindowStartupLocation.CenterScreen;
        Background = new SolidColorBrush(Color.FromRgb(239, 244, 241));
        Icon = LoadIcon();
        Content = _view;
        Loaded += OnLoaded;
        Closed += OnClosed;
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        _router?.Dispose();
        _router = null;
        _view.Dispose();
    }

    private async void OnLoaded(object sender, RoutedEventArgs args)
    {
        try { await InitializeAsync().ConfigureAwait(true); }
        catch (Exception error)
        {
            AppLog.HandleUnhandled(error);
            ShowFallback(error);
        }
    }

    private void OnClosed(object? sender, EventArgs args) => Dispose();

    private async Task InitializeAsync()
    {
        string webDirectory = Path.Combine(AppContext.BaseDirectory, "Ui", "Web");
        if (!Directory.Exists(webDirectory)) throw new DirectoryNotFoundException(webDirectory);
        string userDataFolder = Path.Combine(Path.GetTempPath(), "MinecraftServerDownloader", "WebView2");
        CoreWebView2Environment environment = await CoreWebView2Environment.CreateAsync(null, userDataFolder, new CoreWebView2EnvironmentOptions()).ConfigureAwait(true);
        await _view.EnsureCoreWebView2Async(environment).ConfigureAwait(true);
        CoreWebView2 core = _view.CoreWebView2 ?? throw new InvalidOperationException("WebView2");
        core.SetVirtualHostNameToFolderMapping(VirtualHost, webDirectory, CoreWebView2HostResourceAccessKind.Allow);
        core.Settings.AreDefaultContextMenusEnabled = false;
        core.Settings.IsStatusBarEnabled = false;
        core.Settings.IsZoomControlEnabled = false;
        core.Settings.AreDevToolsEnabled = false;
        core.Settings.AreBrowserAcceleratorKeysEnabled = false;
        core.WebMessageReceived += OnWebMessageReceived;
        core.NewWindowRequested += (_, args) => { args.Handled = true; ExternalLauncher.Open(args.Uri); };
        core.NavigationStarting += (_, args) =>
        {
            if (!args.Uri.StartsWith("https://" + VirtualHost + "/", StringComparison.OrdinalIgnoreCase))
            {
                args.Cancel = true;
                ExternalLauncher.Open(args.Uri);
            }
        };
        core.Navigate(EntryPage);
    }

    private async void OnWebMessageReceived(object? sender, CoreWebView2WebMessageReceivedEventArgs args)
    {
        long id = 0;
        try
        {
            string raw = args.WebMessageAsJson;
            if (raw.Length > MaxMessageCharacters) throw new AppException("bridge.tooLarge");
            using JsonDocument document = JsonDocument.Parse(raw);
            JsonElement root = document.RootElement;
            id = BridgeJson.Int(root, "id");
            string action = BridgeJson.Text(root, "action");
            JsonElement payload = root.TryGetProperty("payload", out JsonElement node) ? node : default;
            _router ??= new CommandRouter(PostEvent);
            object result = await _router.ExecuteAsync(action, payload).ConfigureAwait(true);
            PostResponse(id, true, result, "");
        }
        catch (OperationCanceledException) { PostResponse(id, false, null, "operation.cancelled"); }
        catch (AppValidationException error) { PostResponse(id, false, null, error.Message); }
        catch (JsonException) { AppLog.Warn("bridge.json.invalid"); PostResponse(id, false, null, "bridge.invalidJson"); }
        catch (Exception error)
        {
            AppLog.HandleUnhandled(error);
            PostResponse(id, false, null, "bridge.failed");
        }
    }

    private void PostResponse(long id, bool ok, object? result, string? error)
    {
        if (_view.CoreWebView2 is not CoreWebView2 core) return;
        try { core.PostWebMessageAsJson(BridgeJson.Serialize(new { id, ok, result, error })); }
        catch (InvalidOperationException) { }
    }

    private void PostEvent(object payload)
    {
        if (!Dispatcher.CheckAccess())
        {
            Dispatcher.BeginInvoke(() => PostEvent(payload));
            return;
        }
        if (_view.CoreWebView2 is not CoreWebView2 core) return;
        try { core.PostWebMessageAsJson(BridgeJson.Serialize(payload)); }
        catch (InvalidOperationException) { }
    }

    private void ShowFallback(Exception error)
    {
        bool resourceFailure = error is DirectoryNotFoundException or FileNotFoundException;
        bool chinese = CultureInfo.CurrentUICulture.TwoLetterISOLanguageName.Equals("zh", StringComparison.OrdinalIgnoreCase);
        Border panel = new()
        {
            MaxWidth = 560,
            Padding = new Thickness(28),
            VerticalAlignment = VerticalAlignment.Center,
            HorizontalAlignment = HorizontalAlignment.Center,
            Background = Brushes.White,
            BorderBrush = new SolidColorBrush(Color.FromRgb(210, 222, 214)),
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(8)
        };
        StackPanel content = new();
        content.Children.Add(Text(chinese ? "界面无法启动" : "Interface failed to start", 20, FontWeights.SemiBold));
        content.Children.Add(Text(resourceFailure
            ? chinese ? "界面资源缺失或无法读取，请重新安装程序。" : "Interface resources are missing or unreadable. Reinstall the application."
            : chinese ? "WebView2 运行时不可用，请安装 Microsoft Edge WebView2 Runtime。" : "The Microsoft Edge WebView2 Runtime is unavailable. Install it and reopen the application.", 13, FontWeights.Normal, 12));
        Button retry = new() { Content = chinese ? "重试" : "Retry", HorizontalAlignment = HorizontalAlignment.Left, Margin = new Thickness(0, 18, 0, 0), Padding = new Thickness(16, 7, 16, 7) };
        retry.Click += async (_, _) => { retry.IsEnabled = false; Content = _view; try { await InitializeAsync().ConfigureAwait(true); } catch (Exception failure) { AppLog.HandleUnhandled(failure); retry.IsEnabled = true; ShowFallback(failure); } };
        content.Children.Add(retry);
        panel.Child = content;
        Content = panel;
    }

    private static TextBlock Text(string value, double size, FontWeight weight, double topMargin = 0) => new() { Text = value, FontSize = size, FontWeight = weight, TextWrapping = TextWrapping.Wrap, Margin = new Thickness(0, topMargin, 0, 0) };

    private static BitmapImage? LoadIcon()
    {
        string path = Path.Combine(AppContext.BaseDirectory, "app.ico");
        if (!File.Exists(path)) return null;
        try
        {
            BitmapImage image = new();
            image.BeginInit();
            image.UriSource = new Uri(path, UriKind.Absolute);
            image.CacheOption = BitmapCacheOption.OnLoad;
            image.EndInit();
            image.Freeze();
            return image;
        }
        catch (Exception error) when (error is IOException or NotSupportedException or UriFormatException)
        {
            AppLog.Warn(error.Message);
            return null;
        }
    }
}
