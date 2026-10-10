using System.Windows;
using System.Windows.Threading;
using MinecraftServerDownloader.Core;
using MinecraftServerDownloader.Ui;

namespace MinecraftServerDownloader;

public static class Program
{
    [STAThread]
    public static int Main()
    {
        AppLog.Bind();
        Application application = new() { ShutdownMode = ShutdownMode.OnMainWindowClose };
        application.Dispatcher.UnhandledException += OnDispatcherUnhandled;
        AppDomain.CurrentDomain.UnhandledException += OnDomainUnhandled;
        TaskScheduler.UnobservedTaskException += OnUnobservedTask;
        ConfigStore.Load();
        Window window = new WebViewWindow();
        application.MainWindow = window;
        window.Show();
        return application.Run();
    }

    private static void OnDispatcherUnhandled(object sender, DispatcherUnhandledExceptionEventArgs args)
    {
        AppLog.HandleUnhandled(args.Exception);
        args.Handled = true;
    }

    private static void OnDomainUnhandled(object sender, UnhandledExceptionEventArgs args)
    {
        if (args.ExceptionObject is Exception error) AppLog.HandleUnhandled(error);
    }

    private static void OnUnobservedTask(object? sender, UnobservedTaskExceptionEventArgs args)
    {
        AppLog.HandleUnhandled(args.Exception);
        args.SetObserved();
    }
}
