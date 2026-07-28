using Forms = System.Windows.Forms;

namespace SwitchifyPc.App;

public sealed class NativeTrayIcon : IDisposable
{
    private readonly Forms.NotifyIcon notifyIcon;

    public NativeTrayIcon(
        Action showMainWindow,
        Action showSettingsWindow,
        Action showSwitchControlProfiles,
        Func<string> statusText,
        Func<bool> canDisconnect,
        Action disconnectDevices,
        Action quit)
    {
        Forms.ContextMenuStrip menu = CreateMenu(
            showMainWindow,
            showSettingsWindow,
            showSwitchControlProfiles,
            statusText,
            canDisconnect,
            disconnectDevices,
            quit);

        notifyIcon = new Forms.NotifyIcon
        {
            Icon = LoadAppIcon(),
            Text = "Switchify PC",
            ContextMenuStrip = menu,
            Visible = true
        };
        notifyIcon.DoubleClick += (_, _) => showMainWindow();
    }

    internal static Forms.ContextMenuStrip CreateMenu(
        Action showMainWindow,
        Action showSettingsWindow,
        Action showSwitchControlProfiles,
        Func<string> statusText,
        Func<bool> canDisconnect,
        Action disconnectDevices,
        Action quit)
    {
        Forms.ContextMenuStrip menu = new();
        menu.Items.Add("Show Switchify PC", null, (_, _) => showMainWindow());
        menu.Items.Add("Open settings", null, (_, _) => showSettingsWindow());
        menu.Items.Add("Switch control profiles", null, (_, _) => showSwitchControlProfiles());
        menu.Items.Add(new Forms.ToolStripSeparator());
        Forms.ToolStripMenuItem statusItem = new("Status unavailable")
        {
            Enabled = false
        };
        Forms.ToolStripMenuItem disconnectItem = new("Disconnect devices", null, (_, _) => disconnectDevices());
        menu.Items.Add(statusItem);
        menu.Items.Add(disconnectItem);
        menu.Items.Add(new Forms.ToolStripSeparator());
        menu.Items.Add("Quit", null, (_, _) => quit());
        menu.Opening += (_, _) =>
        {
            statusItem.Text = statusText();
            disconnectItem.Enabled = canDisconnect();
        };

        return menu;
    }

    public void Dispose()
    {
        notifyIcon.Visible = false;
        notifyIcon.ContextMenuStrip?.Dispose();
        notifyIcon.Dispose();
    }

    private static System.Drawing.Icon LoadAppIcon()
    {
        string? processPath = Environment.ProcessPath;
        if (processPath is { Length: > 0 } && System.IO.File.Exists(processPath))
        {
            return System.Drawing.Icon.ExtractAssociatedIcon(processPath) ?? System.Drawing.SystemIcons.Application;
        }

        return System.Drawing.SystemIcons.Application;
    }
}
