using System.Threading;
using SwitchifyPc.App;
using Forms = System.Windows.Forms;

namespace SwitchifyPc.Tests;

public sealed class NativeTrayIconTests
{
    [Fact]
    public void MenuOffersDirectSwitchControlProfilesEntry()
    {
        RunOnSta(() =>
        {
            int mainCalls = 0;
            int settingsCalls = 0;
            int profileCalls = 0;
            int disconnectCalls = 0;
            int quitCalls = 0;
            using Forms.ContextMenuStrip menu = NativeTrayIcon.CreateMenu(
                () => mainCalls++,
                () => settingsCalls++,
                () => profileCalls++,
                () => "Status: Bluetooth ready.",
                () => true,
                () => disconnectCalls++,
                () => quitCalls++);

            Assert.Equal(
                [
                    "Show Switchify PC",
                    "Open settings",
                    "Switch control profiles",
                    "Status unavailable",
                    "Disconnect devices",
                    "Quit"
                ],
                menu.Items.OfType<Forms.ToolStripMenuItem>().Select(item => item.Text));

            Forms.ToolStripMenuItem profileItem = menu.Items
                .OfType<Forms.ToolStripMenuItem>()
                .Single(item => item.Text == "Switch control profiles");
            profileItem.PerformClick();

            Assert.Equal(1, profileCalls);
            Assert.Equal(0, mainCalls);
            Assert.Equal(0, settingsCalls);
            Assert.Equal(0, disconnectCalls);
            Assert.Equal(0, quitCalls);
        });
    }

    private static void RunOnSta(Action action)
    {
        Exception? error = null;
        Thread thread = new(() =>
        {
            try
            {
                action();
            }
            catch (Exception exception)
            {
                error = exception;
            }
        });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        thread.Join();
        if (error is not null) throw error;
    }
}
