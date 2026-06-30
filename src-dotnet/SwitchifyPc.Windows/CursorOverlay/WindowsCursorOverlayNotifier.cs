using System.Drawing;
using System.Drawing.Drawing2D;
using System.Threading;
using SwitchifyPc.Core.Input;
using SwitchifyPc.Core.Settings;
using SwitchifyPc.Windows.Input;
using Forms = System.Windows.Forms;

namespace SwitchifyPc.Windows.CursorOverlay;

public sealed class WindowsCursorOverlayNotifier : ICursorOverlayNotifier, IDisposable
{
    private const int DefaultDurationMs = 900;
    private const int FollowIntervalMs = 75;

    private readonly IWindowsNativeInput nativeInput;
    private readonly ICursorOverlaySettingsStore settingsStore;
    private readonly Lazy<OverlayThread> overlayThread;
    private readonly object sync = new();
    private System.Threading.Timer? followTimer;
    private bool dragActive;
    private bool disposed;

    public WindowsCursorOverlayNotifier(IWindowsNativeInput nativeInput, ICursorOverlaySettingsStore settingsStore)
    {
        this.nativeInput = nativeInput;
        this.settingsStore = settingsStore;
        overlayThread = new Lazy<OverlayThread>(() => new OverlayThread());
    }

    public void Show(string eventName)
    {
        CursorOverlaySettings settings = settingsStore.Load();
        if (!settings.Enabled) return;
        if (ShouldFollowCursor(settings)) StartFollowing();
        ShowCurrent(eventName, settings, persistent: ShouldFollowCursor(settings));
    }

    public void Hide()
    {
        CursorOverlaySettings settings = settingsStore.Load();
        if (settings.Enabled && settings.Visibility == "whileControlling" && IsFollowing())
        {
            return;
        }

        HideOverlay();
    }

    private void HideOverlay()
    {
        StopFollowing();
        if (overlayThread.IsValueCreated)
        {
            overlayThread.Value.Post(form => form.HideOverlay());
        }
    }

    public void EndControlSession()
    {
        lock (sync)
        {
            dragActive = false;
        }

        HideOverlay();
    }

    public void MarkControlActive()
    {
        CursorOverlaySettings settings = settingsStore.Load();
        if (settings.Enabled && ShouldFollowCursor(settings))
        {
            StartFollowing();
        }
    }

    public void SetDragActive(bool active)
    {
        lock (sync)
        {
            if (dragActive == active) return;
            dragActive = active;
        }

        CursorOverlaySettings settings = settingsStore.Load();
        if (!settings.Enabled) return;
        if (active)
        {
            StartFollowing();
        }
        else if (ShouldFollowCursor(settings))
        {
            ShowCurrent("move", settings, persistent: true);
        }
        else
        {
            Hide();
        }
    }

    public void Dispose()
    {
        if (disposed) return;
        disposed = true;
        StopFollowing();
        if (overlayThread.IsValueCreated)
        {
            overlayThread.Value.Dispose();
        }
    }

    private void StartFollowing()
    {
        lock (sync)
        {
            if (followTimer is not null) return;
            followTimer = new System.Threading.Timer(_ =>
            {
                CursorOverlaySettings settings = settingsStore.Load();
                if (!settings.Enabled || !ShouldFollowCursor(settings))
                {
                    HideOverlay();
                    return;
                }

                ShowCurrent(CurrentPersistentEvent(), settings, persistent: true);
            }, null, TimeSpan.Zero, TimeSpan.FromMilliseconds(FollowIntervalMs));
        }
    }

    private void StopFollowing()
    {
        lock (sync)
        {
            followTimer?.Dispose();
            followTimer = null;
        }
    }

    private bool IsFollowing()
    {
        lock (sync)
        {
            return followTimer is not null;
        }
    }

    private void ShowCurrent(string eventName, CursorOverlaySettings settings, bool persistent)
    {
        PointerPosition cursor = nativeInput.GetCursorPosition();
        int[] color = CursorOverlaySettingsModel.ResolveColorRgb(settings.Color);
        OverlayRenderCommand command = new(
            EventName: eventName,
            X: cursor.X,
            Y: cursor.Y,
            Size: CursorOverlaySettingsModel.ResolveSizePixels(settings.Size),
            DurationMs: persistent ? 0 : DefaultDurationMs,
            Crosshairs: settings.Crosshairs,
            Persistent: persistent,
            Color: Color.FromArgb(color[0], color[1], color[2]));
        overlayThread.Value.Post(form => form.ShowOverlay(command));
    }

    private bool ShouldFollowCursor(CursorOverlaySettings settings)
    {
        lock (sync)
        {
            return settings.Visibility == "whileControlling" || dragActive;
        }
    }

    private string CurrentPersistentEvent()
    {
        lock (sync)
        {
            return dragActive ? "drag" : "move";
        }
    }

    private sealed class OverlayThread : IDisposable
    {
        private readonly TaskCompletionSource<OverlayForm> formReady = new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly Thread thread;

        public OverlayThread()
        {
            thread = new Thread(Run)
            {
                IsBackground = true,
                Name = "Switchify cursor overlay"
            };
            thread.SetApartmentState(ApartmentState.STA);
            thread.Start();
        }

        public void Post(Action<OverlayForm> action)
        {
            OverlayForm form = formReady.Task.GetAwaiter().GetResult();
            if (form.IsDisposed) return;
            form.BeginInvoke(() => action(form));
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
            using OverlayForm form = new();
            _ = form.Handle;
            formReady.SetResult(form);
            Forms.Application.Run();
        }
    }

    private sealed record OverlayRenderCommand(
        string EventName,
        double X,
        double Y,
        int Size,
        int DurationMs,
        bool Crosshairs,
        bool Persistent,
        Color Color);

    private sealed class OverlayForm : Forms.Form
    {
        private const int DefaultWindowSize = 128;
        private const int ClickPulseMs = 180;
        private const int CrosshairThickness = 2;
        private readonly Forms.Timer hideTimer = new();
        private readonly Forms.Timer animationTimer = new();
        private readonly CrosshairLineForm horizontalCrosshair = new();
        private readonly CrosshairLineForm verticalCrosshair = new();
        private DateTime clickPulseStartedAt = DateTime.MinValue;
        private bool isClickPulse;
        private bool isDragActive;
        private int ringWindowSize = DefaultWindowSize;
        private PointF cursorCenter = new(DefaultWindowSize / 2.0f, DefaultWindowSize / 2.0f);
        private Color overlayColor = Color.FromArgb(211, 47, 47);

        public OverlayForm()
        {
            AutoScaleMode = Forms.AutoScaleMode.None;
            BackColor = Color.Black;
            ClientSize = new Size(DefaultWindowSize, DefaultWindowSize);
            ControlBox = false;
            FormBorderStyle = Forms.FormBorderStyle.None;
            MaximizeBox = false;
            MinimizeBox = false;
            Name = "SwitchifyCursorOverlay";
            ShowIcon = false;
            ShowInTaskbar = false;
            StartPosition = Forms.FormStartPosition.Manual;
            TopMost = true;

            hideTimer.Tick += (_, _) => HideOverlay();
            animationTimer.Interval = 16;
            animationTimer.Tick += (_, _) => TickAnimation();
        }

        protected override bool ShowWithoutActivation => true;

        protected override CreateParams CreateParams
        {
            get
            {
                CreateParams createParams = base.CreateParams;
                createParams.ExStyle |=
                    CursorOverlayNativeMethods.WS_EX_LAYERED |
                    CursorOverlayNativeMethods.WS_EX_TRANSPARENT |
                    CursorOverlayNativeMethods.WS_EX_TOPMOST |
                    CursorOverlayNativeMethods.WS_EX_TOOLWINDOW |
                    CursorOverlayNativeMethods.WS_EX_NOACTIVATE;
                return createParams;
            }
        }

        public void ShowOverlay(OverlayRenderCommand command)
        {
            int size = command.Size > 0 ? command.Size : DefaultWindowSize;
            int durationMs = command.DurationMs > 0 ? command.DurationMs : DefaultDurationMs;
            System.Drawing.Point cursor = new((int)Math.Round(command.X), (int)Math.Round(command.Y));
            Forms.Screen display = Forms.Screen.FromPoint(cursor);
            Rectangle displayBounds = display.Bounds;
            ringWindowSize = size;
            overlayColor = command.Color;

            ClientSize = new Size(size, size);
            Location = new System.Drawing.Point(
                (int)Math.Round(command.X - size / 2.0),
                (int)Math.Round(command.Y - size / 2.0));
            cursorCenter = new PointF(size / 2.0f, size / 2.0f);

            isClickPulse = string.Equals(command.EventName, "click", StringComparison.OrdinalIgnoreCase);
            isDragActive = string.Equals(command.EventName, "drag", StringComparison.OrdinalIgnoreCase);
            clickPulseStartedAt = isClickPulse ? DateTime.UtcNow : DateTime.MinValue;

            if (!RenderLayeredOverlay())
            {
                HideOverlay();
                return;
            }

            ApplyTopMostNoActivate();
            ShowCrosshairs(command.Crosshairs, cursor, displayBounds, overlayColor);

            hideTimer.Stop();
            if (!command.Persistent)
            {
                hideTimer.Interval = durationMs;
                hideTimer.Start();
            }

            animationTimer.Start();
        }

        public void HideOverlay()
        {
            hideTimer.Stop();
            animationTimer.Stop();
            isClickPulse = false;
            isDragActive = false;
            horizontalCrosshair.Hide();
            verticalCrosshair.Hide();
            Hide();
        }

        private float ResolvePulseScale()
        {
            if (!isClickPulse) return 1.0f;
            double elapsedMs = (DateTime.UtcNow - clickPulseStartedAt).TotalMilliseconds;
            if (elapsedMs >= ClickPulseMs)
            {
                isClickPulse = false;
                return 1.0f;
            }

            double progress = Math.Clamp(elapsedMs / ClickPulseMs, 0.0, 1.0);
            return (float)(0.82 + progress * 0.36);
        }

        private void TickAnimation()
        {
            if (isClickPulse)
            {
                _ = RenderLayeredOverlay();
                return;
            }

            animationTimer.Stop();
        }

        private bool RenderLayeredOverlay()
        {
            using Bitmap bitmap = new(ClientSize.Width, ClientSize.Height, System.Drawing.Imaging.PixelFormat.Format32bppPArgb);
            using (Graphics graphics = Graphics.FromImage(bitmap))
            {
                graphics.SmoothingMode = SmoothingMode.AntiAlias;
                graphics.Clear(Color.Transparent);

                float scale = ResolvePulseScale();
                float ringDiameter = ringWindowSize * 0.5625f * scale;
                float ringStroke = Math.Max(4.0f, ringWindowSize * 0.039f);
                float outerStroke = Math.Max(18.0f, ringWindowSize * 0.1875f) * scale;
                float centerX = cursorCenter.X;
                float centerY = cursorCenter.Y;
                float ringX = centerX - ringDiameter / 2.0f;
                float ringY = centerY - ringDiameter / 2.0f;
                float dragDotDiameter = ringDiameter * 0.22f;
                float dragDotX = centerX - dragDotDiameter / 2.0f;
                float dragDotY = centerY - dragDotDiameter / 2.0f;

                using Pen glow = new(Color.FromArgb(isDragActive ? 66 : 62, overlayColor.R, overlayColor.G, overlayColor.B), isDragActive ? outerStroke * 1.08f : outerStroke);
                using Pen ring = new(Color.FromArgb(250, overlayColor.R, overlayColor.G, overlayColor.B), isDragActive ? ringStroke + 1.0f : ringStroke);
                using SolidBrush dragDot = new(Color.FromArgb(240, overlayColor.R, overlayColor.G, overlayColor.B));

                graphics.DrawEllipse(glow, ringX, ringY, ringDiameter, ringDiameter);
                graphics.DrawEllipse(ring, ringX, ringY, ringDiameter, ringDiameter);
                if (isDragActive)
                {
                    graphics.FillEllipse(dragDot, dragDotX, dragDotY, dragDotDiameter, dragDotDiameter);
                }
            }

            IntPtr screenDc = CursorOverlayNativeMethods.GetDC(IntPtr.Zero);
            IntPtr memoryDc = CursorOverlayNativeMethods.CreateCompatibleDC(screenDc);
            IntPtr bitmapHandle = bitmap.GetHbitmap(Color.FromArgb(0));
            IntPtr oldBitmap = CursorOverlayNativeMethods.SelectObject(memoryDc, bitmapHandle);

            try
            {
                CursorOverlayNativeMethods.Point destination = new(Location.X, Location.Y);
                CursorOverlayNativeMethods.Size size = new(ClientSize.Width, ClientSize.Height);
                CursorOverlayNativeMethods.Point source = new(0, 0);
                CursorOverlayNativeMethods.BlendFunction blend = new()
                {
                    BlendOp = CursorOverlayNativeMethods.AC_SRC_OVER,
                    SourceConstantAlpha = 255,
                    AlphaFormat = CursorOverlayNativeMethods.AC_SRC_ALPHA
                };

                return CursorOverlayNativeMethods.UpdateLayeredWindow(Handle, screenDc, ref destination, ref size, memoryDc, ref source, 0, ref blend, CursorOverlayNativeMethods.ULW_ALPHA);
            }
            finally
            {
                CursorOverlayNativeMethods.SelectObject(memoryDc, oldBitmap);
                CursorOverlayNativeMethods.DeleteObject(bitmapHandle);
                CursorOverlayNativeMethods.DeleteDC(memoryDc);
                CursorOverlayNativeMethods.ReleaseDC(IntPtr.Zero, screenDc);
            }
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

        private void ShowCrosshairs(bool enabled, System.Drawing.Point cursor, Rectangle displayBounds, Color color)
        {
            if (!enabled)
            {
                horizontalCrosshair.Hide();
                verticalCrosshair.Hide();
                return;
            }

            horizontalCrosshair.ShowLine(new Rectangle(displayBounds.Left, cursor.Y - CrosshairThickness / 2, displayBounds.Width, CrosshairThickness), color);
            verticalCrosshair.ShowLine(new Rectangle(cursor.X - CrosshairThickness / 2, displayBounds.Top, CrosshairThickness, displayBounds.Height), color);
        }
    }

    private sealed class CrosshairLineForm : Forms.Form
    {
        public CrosshairLineForm()
        {
            AutoScaleMode = Forms.AutoScaleMode.None;
            BackColor = Color.FromArgb(211, 47, 47);
            ControlBox = false;
            FormBorderStyle = Forms.FormBorderStyle.None;
            MaximizeBox = false;
            MinimizeBox = false;
            Name = "SwitchifyCursorCrosshair";
            Opacity = 0.72;
            ShowIcon = false;
            ShowInTaskbar = false;
            StartPosition = Forms.FormStartPosition.Manual;
            TopMost = true;
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

        public void ShowLine(Rectangle bounds, Color color)
        {
            BackColor = color;
            Bounds = bounds;
            if (!Visible)
            {
                Show();
            }

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
    }
}
