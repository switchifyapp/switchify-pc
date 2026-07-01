using System.Threading;
using SwitchifyPc.App;
using SwitchifyPc.Core.Ui;
using SwitchifyPc.Core.Updates;

namespace SwitchifyPc.Tests;

public sealed class SettingsWindowTests
{
    [Fact]
    public void SettingsWindowLoadsWithUpdateDownloadProgress()
    {
        RunOnSta(() =>
        {
            SettingsViewModel viewModel = new();
            viewModel.SetUpdateState(UpdateState.CreateInitial("0.2.4") with
            {
                Download = new UpdateDownloadProgress(
                    UpdateDownloadStatus.Downloading,
                    20_971_520,
                    52_428_800,
                    40)
            });

            SettingsWindow window = new(viewModel);
            try
            {
                window.Show();
                window.UpdateLayout();
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
