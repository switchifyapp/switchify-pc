using System.Threading;
using System.Windows;
using SwitchifyPc.App;
using SwitchifyPc.Core.Ui;

namespace SwitchifyPc.Tests;

public sealed class MainWindowTests
{
    [Fact]
    public void MainWindowLoadsWithAndroidDownloadPrompt()
    {
        RunOnSta(() =>
        {
            MainWindowViewModel viewModel = new();
            viewModel.SetAndroidDownloadPromptState(dismissed: false, hasPairedDevices: false);
            MainWindow window = new(viewModel);
            try
            {
                window.Show();
                window.UpdateLayout();

                string assemblyName = Uri.EscapeDataString(typeof(MainWindow).Assembly.GetName().Name ?? "Switchify PC");
                Uri qrUri = new($"pack://application:,,,/{assemblyName};component/Assets/android-download-qr.png", UriKind.Absolute);
                Assert.NotNull(System.Windows.Application.GetResourceStream(qrUri));
            }
            finally
            {
                window.Close();
            }
        });
    }

    private static void RunOnSta(Action action)
    {
        Exception? exception = null;
        Thread thread = new(() =>
        {
            try
            {
                action();
            }
            catch (Exception error)
            {
                exception = error;
            }
        });

        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        thread.Join();

        if (exception is not null) throw exception;
    }
}
