using System.Drawing;
using System.Drawing.Drawing2D;
using SwitchifyPc.Core.Input;
using SwitchifyPc.Windows.CursorOverlay;
using SwitchifyPc.Windows.Input;
using Forms = System.Windows.Forms;

namespace SwitchifyPc.Windows.ModifierOverlay;

public sealed class WindowsModifierKeyOverlayNotifier : IModifierKeyOverlayNotifier, IDisposable
{
    private readonly IWindowsNativeInput nativeInput;
    private readonly Lazy<OverlayThread> overlayThread;
    private bool disposed;

    public WindowsModifierKeyOverlayNotifier(IWindowsNativeInput nativeInput)
    {
        this.nativeInput = nativeInput;
        overlayThread = new Lazy<OverlayThread>(() => new OverlayThread(nativeInput));
    }

    public void SetActiveModifiers(IReadOnlyCollection<string> activeModifiers)
    {
        if (disposed) return;
        string[] labels = NormalizeLabels(activeModifiers);
        overlayThread.Value.Post(form => form.SetActiveModifiers(labels));
    }

    public void EndControlSession()
    {
        if (disposed) return;
        if (overlayThread.IsValueCreated)
        {
            overlayThread.Value.Post(form => form.HideOverlay());
        }
    }

    public void Dispose()
    {
        if (disposed) return;
        disposed = true;
        if (overlayThread.IsValueCreated)
        {
            overlayThread.Value.Dispose();
        }
    }

    internal static string[] NormalizeLabels(IEnumerable<string> activeModifiers)
    {
        HashSet<string> labels = new(activeModifiers, StringComparer.Ordinal);
        List<string> ordered = [];
        foreach (string label in new[] { "Ctrl", "Alt", "Shift", "Start" })
        {
            if (labels.Contains(label))
            {
                ordered.Add(label);
            }
        }

        return ordered.ToArray();
    }

    private sealed class OverlayThread : IDisposable
    {
        private readonly IWindowsNativeInput nativeInput;
        private readonly TaskCompletionSource<OverlayForm> formReady = new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly Thread thread;

        public OverlayThread(IWindowsNativeInput nativeInput)
        {
            this.nativeInput = nativeInput;
            thread = new Thread(Run)
            {
                IsBackground = true,
                Name = "Switchify modifier overlay"
            };
            thread.SetApartmentState(ApartmentState.STA);
            thread.Start();
        }

        public void Post(Action<OverlayForm> action)
        {
            OverlayForm form = formReady.Task.GetAwaiter().GetResult();
            if (form.IsDisposed) return;
            form.BeginInvoke(() =>
            {
                try
                {
                    action(form);
                }
                catch
                {
                    form.HideOverlay();
                }
            });
        }

        public void Dispose()
        {
            if (!formReady.Task.IsCompletedSuccessfully) return;
            OverlayForm form = formReady.Task.Result;
            if (!form.IsDisposed)
            {
                form.BeginInvoke(() =>
                {
                    form.HideOverlay();
                    form.Close();
                    Forms.Application.ExitThread();
                });
            }
        }

        private void Run()
        {
            Forms.Application.EnableVisualStyles();
            using OverlayForm form = new(nativeInput);
            _ = form.Handle;
            formReady.SetResult(form);
            Forms.Application.Run();
        }
    }

    private sealed class OverlayForm : Forms.Form
    {
        private const int MarginPx = 16;
        private const int PaddingPx = 16;
        private const int ChipPaddingX = 18;
        private const int ChipHeight = 38;
        private const int GapPx = 10;
        private static readonly Color PanelColor = Color.FromArgb(0x1F, 0x1F, 0x23);
        private static readonly Color BrandRed = Color.FromArgb(0xD3, 0x2F, 0x2F);
        private readonly IWindowsNativeInput nativeInput;
        private readonly Font chipFont;

        public OverlayForm(IWindowsNativeInput nativeInput)
        {
            this.nativeInput = nativeInput;
            AutoScaleMode = Forms.AutoScaleMode.None;
            BackColor = PanelColor;
            ClientSize = new Size(160, 44);
            ControlBox = false;
            FormBorderStyle = Forms.FormBorderStyle.None;
            MaximizeBox = false;
            MinimizeBox = false;
            Name = "SwitchifyModifierOverlay";
            ShowIcon = false;
            ShowInTaskbar = false;
            StartPosition = Forms.FormStartPosition.Manual;
            TopMost = true;
            chipFont = new Font(Font.FontFamily, 10.0f, FontStyle.Bold);
        }

        protected override bool ShowWithoutActivation => true;

        protected override CreateParams CreateParams
        {
            get
            {
                CreateParams createParams = base.CreateParams;
                createParams.ExStyle |=
                    CursorOverlayNativeMethods.WS_EX_TRANSPARENT |
                    CursorOverlayNativeMethods.WS_EX_TOPMOST |
                    CursorOverlayNativeMethods.WS_EX_TOOLWINDOW |
                    CursorOverlayNativeMethods.WS_EX_NOACTIVATE;
                return createParams;
            }
        }

        public void SetActiveModifiers(IReadOnlyList<string> activeModifiers)
        {
            Controls.Clear();
            if (activeModifiers.Count == 0)
            {
                HideOverlay();
                return;
            }

            int x = PaddingPx;
            foreach (string label in activeModifiers)
            {
                Size textSize = Forms.TextRenderer.MeasureText(
                    label,
                    chipFont,
                    Size.Empty,
                    Forms.TextFormatFlags.NoPadding | Forms.TextFormatFlags.SingleLine);
                int width = Math.Max(MinimumChipWidth(label), textSize.Width + ChipPaddingX * 2);
                Forms.Label chip = new()
                {
                    AutoSize = false,
                    BackColor = BrandRed,
                    ForeColor = Color.White,
                    Text = label,
                    TextAlign = ContentAlignment.MiddleCenter,
                    Font = chipFont,
                    Bounds = new Rectangle(x, PaddingPx, width, ChipHeight)
                };
                Controls.Add(chip);
                x += width + GapPx;
            }

            ClientSize = new Size(Math.Max(48, x - GapPx + PaddingPx), ChipHeight + PaddingPx * 2);
            Region?.Dispose();
            Region = new Region(RoundedRectangle(new Rectangle(System.Drawing.Point.Empty, ClientSize), 8));
            Reposition();
            Show();
            ApplyTopMostNoActivate();
        }

        private static int MinimumChipWidth(string label)
        {
            return label switch
            {
                "Ctrl" => 68,
                "Alt" => 60,
                "Shift" => 74,
                "Start" => 74,
                _ => 68
            };
        }

        public void HideOverlay()
        {
            Controls.Clear();
            Hide();
        }

        protected override void OnPaint(Forms.PaintEventArgs e)
        {
            base.OnPaint(e);
            using Pen border = new(BrandRed, 1);
            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
            e.Graphics.DrawPath(border, RoundedRectangle(new Rectangle(0, 0, ClientSize.Width - 1, ClientSize.Height - 1), 8));
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
            {
                chipFont.Dispose();
            }

            base.Dispose(disposing);
        }

        private void Reposition()
        {
            PointerPosition cursor = nativeInput.GetCursorPosition();
            Forms.Screen screen = Forms.Screen.FromPoint(new System.Drawing.Point((int)Math.Round(cursor.X), (int)Math.Round(cursor.Y)));
            Rectangle workArea = screen.WorkingArea;
            Location = new System.Drawing.Point(workArea.Right - Width - MarginPx, workArea.Top + MarginPx);
        }

        private void ApplyTopMostNoActivate()
        {
            CursorOverlayNativeMethods.SetWindowPos(
                Handle,
                CursorOverlayNativeMethods.HWND_TOPMOST,
                0,
                0,
                0,
                0,
                CursorOverlayNativeMethods.SWP_NOMOVE |
                CursorOverlayNativeMethods.SWP_NOSIZE |
                CursorOverlayNativeMethods.SWP_NOACTIVATE |
                CursorOverlayNativeMethods.SWP_SHOWWINDOW);
        }

        private static GraphicsPath RoundedRectangle(Rectangle rectangle, int radius)
        {
            int diameter = radius * 2;
            GraphicsPath path = new();
            path.AddArc(rectangle.Left, rectangle.Top, diameter, diameter, 180, 90);
            path.AddArc(rectangle.Right - diameter, rectangle.Top, diameter, diameter, 270, 90);
            path.AddArc(rectangle.Right - diameter, rectangle.Bottom - diameter, diameter, diameter, 0, 90);
            path.AddArc(rectangle.Left, rectangle.Bottom - diameter, diameter, diameter, 90, 90);
            path.CloseFigure();
            return path;
        }
    }
}
