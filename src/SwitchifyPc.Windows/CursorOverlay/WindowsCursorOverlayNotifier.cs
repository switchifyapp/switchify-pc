using System.Drawing;
using System.Drawing.Drawing2D;
using System.Threading;
using SwitchifyPc.Core.Input;
using SwitchifyPc.Core.Settings;
using SwitchifyPc.Windows.Input;
using Forms = System.Windows.Forms;

namespace SwitchifyPc.Windows.CursorOverlay;

public sealed class WindowsCursorOverlayNotifier : ICursorOverlayNotifier, IMouseRepeatFeedbackNotifier, IDisposable
{
    private const int DefaultDurationMs = 900;
    private const int FollowIntervalMs = 75;
    private const int SettingsCacheTtlMs = 1000;

    private readonly IWindowsNativeInput nativeInput;
    private readonly ICursorOverlaySettingsStore settingsStore;
    private readonly Func<bool> animationsEnabled;
    private readonly Func<PointerPosition, double> displayScale;
    private readonly Func<double> now;
    private readonly Action<string> warn;
    private readonly Lazy<OverlayThread> overlayThread;
    private readonly object sync = new();
    private System.Threading.Timer? followTimer;
    private CursorOverlaySettings? cachedSettings;
    private double cachedSettingsAtMs = double.NegativeInfinity;
    private double lastMoveRenderAtMs = double.NegativeInfinity;
    private bool dragActive;
    private int renderingDisabled;
    private bool disposed;

    public WindowsCursorOverlayNotifier(
        IWindowsNativeInput nativeInput,
        ICursorOverlaySettingsStore settingsStore,
        Func<double>? now = null,
        Func<bool>? animationsEnabled = null,
        Func<PointerPosition, double>? displayScale = null,
        Action<string>? warn = null)
    {
        this.nativeInput = nativeInput;
        this.settingsStore = settingsStore;
        this.animationsEnabled = animationsEnabled ?? new WindowsCursorOverlayMotionPolicy().AnimationsEnabled;
        this.displayScale = displayScale ?? (position => CursorOverlayDpi.ScaleForPoint(position.X, position.Y));
        this.now = now ?? (() => DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());
        this.warn = warn ?? Console.WriteLine;
        overlayThread = new Lazy<OverlayThread>(() => new OverlayThread(HandleRenderFailure));
    }

    public void Show(CursorOverlayEvent cursorEvent)
    {
        if (RenderingDisabled()) return;
        CursorOverlaySettings settings = CurrentSettings();
        if (!settings.Enabled) return;
        bool shouldFollow = ShouldFollowCursor(settings);
        if (shouldFollow) StartFollowing();
        if (shouldFollow && IsMoveEvent(cursorEvent.Kind) && !ShouldRenderMoveNow()) return;
        ShowCurrent(cursorEvent, settings, persistent: shouldFollow);
    }

    public void Hide()
    {
        CursorOverlaySettings settings = CurrentSettings();
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
        if (RenderingDisabled()) return;
        CursorOverlaySettings settings = CurrentSettings();
        if (settings.Enabled && ShouldFollowCursor(settings))
        {
            StartFollowing();
        }
    }

    public void SetDragActive(bool active)
    {
        if (RenderingDisabled()) return;
        lock (sync)
        {
            if (dragActive == active) return;
            dragActive = active;
        }

        CursorOverlaySettings settings = CurrentSettings();
        if (!settings.Enabled) return;
        if (active)
        {
            StartFollowing();
        }
        else if (ShouldFollowCursor(settings))
        {
            ShowCurrent(new CursorOverlayEvent(CursorOverlayEventKind.Move), settings, persistent: true);
        }
        else
        {
            Hide();
        }
    }

    public void BeginRepeat(MouseRepeatFeedback feedback)
    {
        if (RenderingDisabled()) return;
        CursorOverlaySettings settings = CurrentSettings();
        if (!settings.Enabled) return;
        bool restorePersistentMarker = ShouldFollowCursor(settings);
        if (restorePersistentMarker) StartFollowing();
        CursorOverlayEvent cursorEvent = new(
            feedback.Kind == MouseRepeatFeedbackKind.Scroll
                ? CursorOverlayEventKind.Scroll
                : CursorOverlayEventKind.Move,
            Dx: feedback.Dx,
            Dy: feedback.Dy);
        OverlayRenderCommand command = CreateRenderCommand(
            cursorEvent,
            settings,
            persistent: true);
        overlayThread.Value.Post(form => form.BeginRepeat(command, feedback));
    }

    public void EndRepeat(Guid generationId)
    {
        CursorOverlaySettings settings = CurrentSettings();
        bool restorePersistentMarker = settings.Enabled && ShouldFollowCursor(settings);
        if (overlayThread.IsValueCreated)
        {
            overlayThread.Value.Post(form => form.EndRepeat(generationId, restorePersistentMarker));
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
        if (RenderingDisabled()) return;
        lock (sync)
        {
            if (followTimer is not null) return;
            followTimer = new System.Threading.Timer(_ =>
            {
                if (RenderingDisabled()) return;
                CursorOverlaySettings settings = CurrentSettings();
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

    private bool RenderingDisabled() => Volatile.Read(ref renderingDisabled) != 0;

    private void HandleRenderFailure(Exception error)
    {
        if (Interlocked.Exchange(ref renderingDisabled, 1) != 0) return;
        StopFollowing();
        try
        {
            warn(error.GetType().Name);
        }
        catch
        {
        }
    }

    private void ShowCurrent(CursorOverlayEvent cursorEvent, CursorOverlaySettings settings, bool persistent)
    {
        OverlayRenderCommand command = CreateRenderCommand(cursorEvent, settings, persistent);
        overlayThread.Value.Post(form => form.ShowOverlay(command));
    }

    private OverlayRenderCommand CreateRenderCommand(
        CursorOverlayEvent cursorEvent,
        CursorOverlaySettings settings,
        bool persistent)
    {
        PointerPosition cursor = nativeInput.GetCursorPosition();
        int[] color = CursorOverlaySettingsModel.ResolveColorRgb(settings.Color);
        CursorOverlayVisualTokens tokens = CursorOverlayVisualTokens.Create(
            CursorOverlaySettingsModel.ResolveSizePixels(settings.Size),
            displayScale(cursor));
        OverlayRenderCommand command = new(
            CursorEvent: cursorEvent,
            X: cursor.X,
            Y: cursor.Y,
            Tokens: tokens,
            DurationMs: persistent ? 0 : DurationFor(cursorEvent.Kind),
            Crosshairs: settings.Crosshairs,
            Persistent: persistent,
            AnimationsEnabled: animationsEnabled(),
            Color: Color.FromArgb(color[0], color[1], color[2]));
        return command;
    }

    private bool ShouldFollowCursor(CursorOverlaySettings settings)
    {
        lock (sync)
        {
            return settings.Visibility == "whileControlling" || dragActive;
        }
    }

    private CursorOverlayEvent CurrentPersistentEvent()
    {
        lock (sync)
        {
            return new CursorOverlayEvent(
                dragActive ? CursorOverlayEventKind.Drag : CursorOverlayEventKind.Move);
        }
    }

    private CursorOverlaySettings CurrentSettings()
    {
        double currentTime = now();
        lock (sync)
        {
            if (cachedSettings is not null && currentTime - cachedSettingsAtMs < SettingsCacheTtlMs)
            {
                return cachedSettings;
            }

            cachedSettings = settingsStore.Load();
            cachedSettingsAtMs = currentTime;
            return cachedSettings;
        }
    }

    private bool ShouldRenderMoveNow()
    {
        double currentTime = now();
        lock (sync)
        {
            if (currentTime - lastMoveRenderAtMs < FollowIntervalMs)
            {
                return false;
            }

            lastMoveRenderAtMs = currentTime;
            return true;
        }
    }

    private static bool IsMoveEvent(CursorOverlayEventKind kind)
    {
        return kind is CursorOverlayEventKind.Move or CursorOverlayEventKind.Drag;
    }

    private static int DurationFor(CursorOverlayEventKind kind)
    {
        return kind switch
        {
            CursorOverlayEventKind.Click => CursorOverlayFeedbackTiming.LandingDurationMs,
            CursorOverlayEventKind.DoubleClick => CursorOverlayFeedbackTiming.DoubleClickDurationMs,
            CursorOverlayEventKind.Scroll => CursorOverlayFeedbackTiming.LandingDurationMs,
            _ => DefaultDurationMs
        };
    }

    private sealed class OverlayThread : IDisposable
    {
        private readonly Action<Exception> onRenderFailure;
        private readonly TaskCompletionSource<OverlayForm> formReady = new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly Thread thread;

        public OverlayThread(Action<Exception> onRenderFailure)
        {
            this.onRenderFailure = onRenderFailure;
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
            try
            {
                form.BeginInvoke(() => form.ExecuteSafely(() => action(form)));
            }
            catch (Exception error)
            {
                onRenderFailure(error);
            }
        }

        public void Dispose()
        {
            if (!formReady.Task.IsCompletedSuccessfully) return;
            OverlayForm form = formReady.Task.Result;
            if (!form.IsDisposed)
            {
                form.BeginInvoke(() =>
                {
                    try
                    {
                        form.HideOverlay();
                    }
                    finally
                    {
                        form.Close();
                        Forms.Application.ExitThread();
                    }
                });
            }
        }

        private void Run()
        {
            Forms.Application.EnableVisualStyles();
            using OverlayForm form = new(onRenderFailure);
            _ = form.Handle;
            formReady.SetResult(form);
            Forms.Application.Run();
        }
    }

    private sealed record OverlayRenderCommand(
        CursorOverlayEvent CursorEvent,
        double X,
        double Y,
        CursorOverlayVisualTokens Tokens,
        int DurationMs,
        bool Crosshairs,
        bool Persistent,
        bool AnimationsEnabled,
        Color Color);

    private sealed class OverlayForm : Forms.Form
    {
        private const int AnimationIntervalMs = 33;
        private readonly Forms.Timer animationTimer = new();
        private readonly CrosshairLineForm horizontalCrosshair = new();
        private readonly CrosshairLineForm verticalCrosshair = new();
        private readonly CursorOverlayGenerationTracker generations = new();
        private readonly CursorOverlayRenderFailureGuard renderFailureGuard = new();
        private readonly Action<Exception> onRenderFailure;
        private CancellationTokenSource? hideCancellation;
        private CancellationTokenSource? pulseCancellation;
        private DateTime landingStartedAt = DateTime.MinValue;
        private bool isLandingSequenceActive;
        private bool isLandingVisible;
        private bool isLandingAnimated;
        private DateTime scrollStartedAt = DateTime.MinValue;
        private bool isScrollSequenceActive;
        private bool isScrollVisible;
        private bool isScrollAnimated;
        private double scrollDx;
        private double scrollDy;
        private MouseRepeatFeedback? activeRepeat;
        private DateTime repeatStartedAt = DateTime.MinValue;
        private bool repeatAnimationsEnabled;
        private bool isDragActive;
        private bool restorePersistentAfterTransient;
        private OverlayRenderCommand? pendingPersistentCommand;
        private CursorOverlayVisualTokens visualTokens = CursorOverlayVisualTokens.Create(128, 1);
        private PointF cursorCenter = new(64, 64);
        private Color overlayColor = Color.FromArgb(211, 47, 47);

        public OverlayForm(Action<Exception> onRenderFailure)
        {
            this.onRenderFailure = onRenderFailure;
            AutoScaleMode = Forms.AutoScaleMode.None;
            BackColor = Color.Black;
            ClientSize = new Size(visualTokens.WindowSize, visualTokens.WindowSize);
            ControlBox = false;
            FormBorderStyle = Forms.FormBorderStyle.None;
            MaximizeBox = false;
            MinimizeBox = false;
            Name = "SwitchifyCursorOverlay";
            ShowIcon = false;
            ShowInTaskbar = false;
            StartPosition = Forms.FormStartPosition.Manual;
            TopMost = true;

            animationTimer.Interval = AnimationIntervalMs;
            animationTimer.Tick += (_, _) => ExecuteSafely(TickAnimation);
        }

        public void ExecuteSafely(Action action)
        {
            _ = renderFailureGuard.TryRun(action, HandleRenderFailure);
        }

        private void HandleRenderFailure(Exception error)
        {
            try
            {
                HideOverlay();
            }
            catch
            {
            }

            try
            {
                onRenderFailure(error);
            }
            catch
            {
            }
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
            if (activeRepeat is not null &&
                command.CursorEvent.Kind is CursorOverlayEventKind.Move or CursorOverlayEventKind.Drag or CursorOverlayEventKind.Scroll)
            {
                (System.Drawing.Point activeCursor, Rectangle activeDisplayBounds) = ApplyCommandFrame(command);
                isDragActive = command.CursorEvent.Kind == CursorOverlayEventKind.Drag;
                if (command.CursorEvent.Kind == CursorOverlayEventKind.Scroll)
                {
                    scrollDx = command.CursorEvent.Dx;
                    scrollDy = command.CursorEvent.Dy;
                }

                RenderLayeredOverlay();
                ApplyTopMostNoActivate();
                ShowCrosshairs(command.Crosshairs, activeCursor, activeDisplayBounds, overlayColor, visualTokens.CrosshairThickness);
                return;
            }

            if ((isLandingSequenceActive || isScrollSequenceActive) && command.Persistent &&
                command.CursorEvent.Kind is CursorOverlayEventKind.Move or CursorOverlayEventKind.Drag)
            {
                pendingPersistentCommand = command;
                return;
            }

            pendingPersistentCommand = null;
            long generation = generations.Next();
            CancelScheduledHide();
            CancelPulseSchedule();
            activeRepeat = null;
            int durationMs = command.DurationMs > 0 ? command.DurationMs : DefaultDurationMs;
            (System.Drawing.Point cursor, Rectangle displayBounds) = ApplyCommandFrame(command);

            bool isClick = command.CursorEvent.Kind is CursorOverlayEventKind.Click or CursorOverlayEventKind.DoubleClick;
            bool isScroll = command.CursorEvent.Kind == CursorOverlayEventKind.Scroll;
            restorePersistentAfterTransient = command.Persistent;
            isLandingSequenceActive = isClick;
            isLandingVisible = isClick;
            isLandingAnimated = isClick && command.AnimationsEnabled;
            isScrollSequenceActive = isScroll;
            isScrollVisible = isScroll;
            isScrollAnimated = isScroll && command.AnimationsEnabled;
            scrollStartedAt = isScrollAnimated ? DateTime.UtcNow : DateTime.MinValue;
            scrollDx = command.CursorEvent.Dx;
            scrollDy = command.CursorEvent.Dy;
            isDragActive = command.CursorEvent.Kind == CursorOverlayEventKind.Drag;
            landingStartedAt = isLandingAnimated ? DateTime.UtcNow : DateTime.MinValue;

            RenderLayeredOverlay();

            ApplyTopMostNoActivate();
            ShowCrosshairs(command.Crosshairs, cursor, displayBounds, overlayColor, visualTokens.CrosshairThickness);

            if (!command.Persistent)
            {
                ScheduleHide(durationMs, generation);
            }

            if (command.CursorEvent.Kind == CursorOverlayEventKind.DoubleClick)
            {
                ScheduleDoubleClick(command.AnimationsEnabled, generation);
            }
            else if (isClick && !command.AnimationsEnabled)
            {
                ScheduleStaticLandingClear(generation);
            }
            else if (isScroll && !command.AnimationsEnabled)
            {
                ScheduleStaticScrollClear(generation);
            }

            if (isLandingAnimated || isScrollAnimated)
            {
                animationTimer.Start();
            }
            else
            {
                animationTimer.Stop();
            }
        }

        public void BeginRepeat(
            OverlayRenderCommand command,
            MouseRepeatFeedback feedback)
        {
            generations.Next();
            CancelScheduledHide();
            CancelPulseSchedule();
            pendingPersistentCommand = null;
            isLandingSequenceActive = false;
            isLandingVisible = false;
            isLandingAnimated = false;
            isScrollSequenceActive = false;
            isScrollVisible = false;
            isScrollAnimated = false;
            activeRepeat = feedback;
            repeatStartedAt = DateTime.UtcNow;
            repeatAnimationsEnabled = command.AnimationsEnabled;
            scrollDx = feedback.Dx;
            scrollDy = feedback.Dy;
            isDragActive = false;
            (System.Drawing.Point cursor, Rectangle displayBounds) = ApplyCommandFrame(command);

            RenderLayeredOverlay();

            ApplyTopMostNoActivate();
            ShowCrosshairs(command.Crosshairs, cursor, displayBounds, overlayColor, visualTokens.CrosshairThickness);
            if (feedback.Kind == MouseRepeatFeedbackKind.Move &&
                feedback.AccelerationDurationMs > 0 &&
                repeatAnimationsEnabled)
            {
                animationTimer.Start();
            }
            else
            {
                animationTimer.Stop();
            }
        }

        public void EndRepeat(Guid generationId, bool restorePersistentMarker)
        {
            if (!CursorOverlayRepeatOwnership.CanEnd(activeRepeat, generationId)) return;
            generations.Invalidate();
            activeRepeat = null;
            animationTimer.Stop();
            if (!restorePersistentMarker)
            {
                HideOverlay();
                return;
            }

            RenderLayeredOverlay();
            ApplyTopMostNoActivate();
        }

        public void HideOverlay()
        {
            generations.Invalidate();
            CancelScheduledHide();
            CancelPulseSchedule();
            animationTimer.Stop();
            isLandingSequenceActive = false;
            isLandingVisible = false;
            isLandingAnimated = false;
            isScrollSequenceActive = false;
            isScrollVisible = false;
            isScrollAnimated = false;
            activeRepeat = null;
            isDragActive = false;
            pendingPersistentCommand = null;
            restorePersistentAfterTransient = false;
            horizontalCrosshair.Hide();
            verticalCrosshair.Hide();
            Hide();
        }

        private (System.Drawing.Point Cursor, Rectangle DisplayBounds) ApplyCommandFrame(OverlayRenderCommand command)
        {
            int size = command.Tokens.WindowSize;
            System.Drawing.Point cursor = new((int)Math.Round(command.X), (int)Math.Round(command.Y));
            visualTokens = command.Tokens;
            overlayColor = command.Color;
            ClientSize = new Size(size, size);
            Location = new System.Drawing.Point(
                (int)Math.Round(command.X - size / 2.0),
                (int)Math.Round(command.Y - size / 2.0));
            cursorCenter = new PointF(size / 2.0f, size / 2.0f);
            return (cursor, Forms.Screen.FromPoint(cursor).Bounds);
        }

        private void TickAnimation()
        {
            if (isLandingAnimated)
            {
                if ((DateTime.UtcNow - landingStartedAt).TotalMilliseconds >= CursorOverlayFeedbackTiming.LandingDurationMs)
                {
                    ClearLanding();
                    return;
                }

                RenderLayeredOverlay();
                return;
            }

            if (isScrollAnimated)
            {
                if ((DateTime.UtcNow - scrollStartedAt).TotalMilliseconds >= CursorOverlayFeedbackTiming.LandingDurationMs)
                {
                    ClearScroll();
                    return;
                }

                RenderLayeredOverlay();
                return;
            }

            if (activeRepeat is { Kind: MouseRepeatFeedbackKind.Move, AccelerationDurationMs: > 0 } repeat &&
                repeatAnimationsEnabled)
            {
                RenderLayeredOverlay();
                if ((DateTime.UtcNow - repeatStartedAt).TotalMilliseconds >= repeat.AccelerationDurationMs)
                {
                    animationTimer.Stop();
                }
                return;
            }

            animationTimer.Stop();
        }

        private void RenderLayeredOverlay()
        {
            using Bitmap bitmap = new(ClientSize.Width, ClientSize.Height, System.Drawing.Imaging.PixelFormat.Format32bppPArgb);
            using (Graphics graphics = Graphics.FromImage(bitmap))
            {
                graphics.SmoothingMode = SmoothingMode.AntiAlias;
                graphics.Clear(Color.Transparent);

                if (activeRepeat is { Kind: MouseRepeatFeedbackKind.Scroll })
                {
                    DrawCursorMarker(graphics);
                    DrawScroll(graphics, 1);
                }
                else if (activeRepeat is { Kind: MouseRepeatFeedbackKind.Move } repeat)
                {
                    DrawCursorMarker(graphics);
                    if (repeat.AccelerationDurationMs > 0)
                    {
                        DrawRepeatProgress(graphics, repeat);
                    }
                }
                else if (isLandingVisible)
                {
                    DrawLanding(graphics);
                }
                else if (isScrollVisible)
                {
                    DrawCursorMarker(graphics);
                    float opacity = isScrollAnimated
                        ? CursorOverlayLandingFrame.Resolve(
                            (DateTime.UtcNow - scrollStartedAt).TotalMilliseconds).Opacity
                        : 1;
                    DrawScroll(graphics, opacity);
                }
                else
                {
                    DrawCursorMarker(graphics);
                }
            }

            IntPtr screenDc = IntPtr.Zero;
            IntPtr memoryDc = IntPtr.Zero;
            IntPtr bitmapHandle = IntPtr.Zero;
            IntPtr oldBitmap = IntPtr.Zero;

            try
            {
                screenDc = CursorOverlayNativeMethods.GetDC(IntPtr.Zero);
                if (screenDc == IntPtr.Zero) throw new InvalidOperationException("Could not acquire the cursor overlay screen context.");
                memoryDc = CursorOverlayNativeMethods.CreateCompatibleDC(screenDc);
                if (memoryDc == IntPtr.Zero) throw new InvalidOperationException("Could not create the cursor overlay memory context.");
                bitmapHandle = bitmap.GetHbitmap(Color.FromArgb(0));
                if (bitmapHandle == IntPtr.Zero) throw new InvalidOperationException("Could not create the cursor overlay bitmap.");
                oldBitmap = CursorOverlayNativeMethods.SelectObject(memoryDc, bitmapHandle);
                if (oldBitmap == IntPtr.Zero || oldBitmap == new IntPtr(-1)) throw new InvalidOperationException("Could not select the cursor overlay bitmap.");

                CursorOverlayNativeMethods.Point destination = new(Location.X, Location.Y);
                CursorOverlayNativeMethods.Size size = new(ClientSize.Width, ClientSize.Height);
                CursorOverlayNativeMethods.Point source = new(0, 0);
                CursorOverlayNativeMethods.BlendFunction blend = new()
                {
                    BlendOp = CursorOverlayNativeMethods.AC_SRC_OVER,
                    SourceConstantAlpha = 255,
                    AlphaFormat = CursorOverlayNativeMethods.AC_SRC_ALPHA
                };

                if (!CursorOverlayNativeMethods.UpdateLayeredWindow(Handle, screenDc, ref destination, ref size, memoryDc, ref source, 0, ref blend, CursorOverlayNativeMethods.ULW_ALPHA))
                {
                    throw new InvalidOperationException("Could not update the cursor overlay window.");
                }
            }
            finally
            {
                if (memoryDc != IntPtr.Zero && oldBitmap != IntPtr.Zero && oldBitmap != new IntPtr(-1))
                {
                    CursorOverlayNativeMethods.SelectObject(memoryDc, oldBitmap);
                }
                if (bitmapHandle != IntPtr.Zero)
                {
                    CursorOverlayNativeMethods.DeleteObject(bitmapHandle);
                }
                if (memoryDc != IntPtr.Zero)
                {
                    CursorOverlayNativeMethods.DeleteDC(memoryDc);
                }
                if (screenDc != IntPtr.Zero)
                {
                    CursorOverlayNativeMethods.ReleaseDC(IntPtr.Zero, screenDc);
                }
            }
        }

        private void DrawCursorMarker(Graphics graphics)
        {
            float ringDiameter = visualTokens.RingDiameter;
            float ringStroke = visualTokens.RingStroke;
            float outerStroke = visualTokens.GlowStroke;
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

        private void DrawLanding(Graphics graphics)
        {
            CursorOverlayLandingFrame frame = isLandingAnimated
                ? CursorOverlayLandingFrame.Resolve((DateTime.UtcNow - landingStartedAt).TotalMilliseconds)
                : CursorOverlayLandingFrame.Static;
            float centerX = cursorCenter.X;
            float centerY = cursorCenter.Y;
            float coreDiameter = visualTokens.LandingCoreDiameter * frame.CoreScale;
            float coreRadius = coreDiameter / 2;
            float maxHaloRadius = (visualTokens.LandingHaloDiameter - visualTokens.LandingHaloStroke) / 2;
            float haloRadius = visualTokens.LandingCoreDiameter / 2 +
                ((maxHaloRadius - visualTokens.LandingCoreDiameter / 2) * frame.HaloProgress);
            int haloAlpha = Alpha(180, frame.Opacity);
            int shadowAlpha = Alpha(65, frame.Opacity);
            int coreAlpha = Alpha(255, frame.Opacity);

            using Pen halo = new(
                Color.FromArgb(haloAlpha, overlayColor.R, overlayColor.G, overlayColor.B),
                visualTokens.LandingHaloStroke);
            using SolidBrush shadow = new(Color.FromArgb(shadowAlpha, 0, 0, 0));
            using SolidBrush core = new(Color.FromArgb(coreAlpha, overlayColor.R, overlayColor.G, overlayColor.B));
            graphics.DrawEllipse(
                halo,
                centerX - haloRadius,
                centerY - haloRadius,
                haloRadius * 2,
                haloRadius * 2);
            graphics.FillEllipse(
                shadow,
                centerX - coreRadius + visualTokens.ShadowOffset,
                centerY - coreRadius + visualTokens.ShadowOffset,
                coreDiameter,
                coreDiameter);
            graphics.FillEllipse(
                core,
                centerX - coreRadius,
                centerY - coreRadius,
                coreDiameter,
                coreDiameter);
        }

        private static int Alpha(int alpha, float opacity) =>
            Math.Clamp((int)Math.Round(alpha * opacity), 0, 255);

        private void DrawScroll(Graphics graphics, float opacity)
        {
            CursorOverlayDirection direction = CursorOverlayDirection.Resolve(scrollDx, scrollDy);
            float directionX = direction.X;
            float directionY = direction.Y;
            float perpendicularX = -directionY;
            float perpendicularY = directionX;
            float halfTrack = visualTokens.ScrollTrackLength / 2;
            float startX = cursorCenter.X - (directionX * halfTrack);
            float startY = cursorCenter.Y - (directionY * halfTrack);
            float endX = cursorCenter.X + (directionX * halfTrack);
            float endY = cursorCenter.Y + (directionY * halfTrack);
            float backX = endX - (directionX * visualTokens.ScrollHeadSize);
            float backY = endY - (directionY * visualTokens.ScrollHeadSize);
            float wing = visualTokens.ScrollHeadSize * 0.55f;

            using Pen underlay = new(
                Color.FromArgb(Alpha(80, opacity), 0, 0, 0),
                visualTokens.ScrollStroke + visualTokens.ShadowOffset)
            {
                StartCap = LineCap.Round,
                EndCap = LineCap.Round,
                LineJoin = LineJoin.Round
            };
            using Pen track = new(
                Color.FromArgb(Alpha(230, opacity), overlayColor.R, overlayColor.G, overlayColor.B),
                visualTokens.ScrollStroke)
            {
                StartCap = LineCap.Round,
                EndCap = LineCap.Round,
                LineJoin = LineJoin.Round
            };

            graphics.DrawLine(underlay, startX, startY, endX, endY);
            graphics.DrawLine(underlay, endX, endY, backX + (perpendicularX * wing), backY + (perpendicularY * wing));
            graphics.DrawLine(underlay, endX, endY, backX - (perpendicularX * wing), backY - (perpendicularY * wing));
            graphics.DrawLine(track, startX, startY, endX, endY);
            graphics.DrawLine(track, endX, endY, backX + (perpendicularX * wing), backY + (perpendicularY * wing));
            graphics.DrawLine(track, endX, endY, backX - (perpendicularX * wing), backY - (perpendicularY * wing));
        }

        private void DrawRepeatProgress(Graphics graphics, MouseRepeatFeedback repeat)
        {
            float diameter = visualTokens.ProgressDiameter;
            float x = cursorCenter.X - (diameter / 2);
            float y = cursorCenter.Y - (diameter / 2);
            using Pen track = new(
                Color.FromArgb(70, overlayColor.R, overlayColor.G, overlayColor.B),
                visualTokens.ProgressStroke)
            {
                StartCap = LineCap.Round,
                EndCap = LineCap.Round
            };
            graphics.DrawEllipse(track, x, y, diameter, diameter);

            if (!repeatAnimationsEnabled)
            {
                using Pen staticRing = new(
                    Color.FromArgb(210, overlayColor.R, overlayColor.G, overlayColor.B),
                    visualTokens.ProgressStroke)
                {
                    DashStyle = DashStyle.Dot
                };
                graphics.DrawEllipse(staticRing, x, y, diameter, diameter);
                return;
            }

            float progress = CursorOverlayRepeatProgress.Resolve(
                repeat.AccelerationDurationMs,
                DateTime.UtcNow - repeatStartedAt);
            float? sweep = CursorOverlayRepeatArc.Sweep(progress);
            if (sweep is null) return;
            using Pen progressRing = new(
                Color.FromArgb(245, overlayColor.R, overlayColor.G, overlayColor.B),
                visualTokens.ProgressStroke)
            {
                StartCap = LineCap.Round,
                EndCap = LineCap.Round
            };
            graphics.DrawArc(progressRing, x, y, diameter, diameter, -90, sweep.Value);
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

        private void ShowCrosshairs(bool enabled, System.Drawing.Point cursor, Rectangle displayBounds, Color color, int thickness)
        {
            if (!enabled)
            {
                horizontalCrosshair.Hide();
                verticalCrosshair.Hide();
                return;
            }

            horizontalCrosshair.ShowLine(new Rectangle(displayBounds.Left, cursor.Y - thickness / 2, displayBounds.Width, thickness), color);
            verticalCrosshair.ShowLine(new Rectangle(cursor.X - thickness / 2, displayBounds.Top, thickness, displayBounds.Height), color);
        }

        private void ScheduleDoubleClick(bool animated, long generation)
        {
            CancellationTokenSource cancellation = new();
            pulseCancellation = cancellation;
            _ = animated
                ? RunAnimatedSecondPulseAsync(generation, cancellation.Token)
                : RunStaticDoubleClickAsync(generation, cancellation.Token);
        }

        private async Task RunAnimatedSecondPulseAsync(long generation, CancellationToken cancellationToken)
        {
            if (!await DelayPulseAsync(CursorOverlayFeedbackTiming.DoubleClickIntervalMs, cancellationToken)) return;
            PostIfCurrent(generation, () =>
            {
                isLandingVisible = true;
                isLandingAnimated = true;
                landingStartedAt = DateTime.UtcNow;
                RenderLayeredOverlay();
                animationTimer.Start();
            });
        }

        private async Task RunStaticDoubleClickAsync(long generation, CancellationToken cancellationToken)
        {
            if (!await DelayPulseAsync(CursorOverlayFeedbackTiming.StaticDoubleClickGapStartsMs, cancellationToken)) return;
            PostIfCurrent(generation, () =>
            {
                isLandingVisible = false;
                RenderLayeredOverlay();
            });
            int gapMs = CursorOverlayFeedbackTiming.DoubleClickIntervalMs -
                CursorOverlayFeedbackTiming.StaticDoubleClickGapStartsMs;
            if (!await DelayPulseAsync(gapMs, cancellationToken)) return;
            PostIfCurrent(generation, () =>
            {
                isLandingVisible = true;
                RenderLayeredOverlay();
            });
            if (!await DelayPulseAsync(CursorOverlayFeedbackTiming.LandingDurationMs, cancellationToken)) return;
            PostIfCurrent(generation, ClearLanding);
        }

        private void ScheduleStaticLandingClear(long generation)
        {
            CancellationTokenSource cancellation = new();
            pulseCancellation = cancellation;
            _ = RunStaticLandingClearAsync(generation, cancellation.Token);
        }

        private void ScheduleStaticScrollClear(long generation)
        {
            CancellationTokenSource cancellation = new();
            pulseCancellation = cancellation;
            _ = RunStaticScrollClearAsync(generation, cancellation.Token);
        }

        private async Task RunStaticLandingClearAsync(long generation, CancellationToken cancellationToken)
        {
            if (!await DelayPulseAsync(CursorOverlayFeedbackTiming.LandingDurationMs, cancellationToken)) return;
            PostIfCurrent(generation, ClearLanding);
        }

        private async Task RunStaticScrollClearAsync(long generation, CancellationToken cancellationToken)
        {
            if (!await DelayPulseAsync(CursorOverlayFeedbackTiming.LandingDurationMs, cancellationToken)) return;
            PostIfCurrent(generation, ClearScroll);
        }

        private static async Task<bool> DelayPulseAsync(int durationMs, CancellationToken cancellationToken)
        {
            try
            {
                await Task.Delay(durationMs, cancellationToken).ConfigureAwait(false);
                return !cancellationToken.IsCancellationRequested;
            }
            catch (OperationCanceledException)
            {
                return false;
            }
        }

        private void PostIfCurrent(long generation, Action action)
        {
            if (IsDisposed) return;
            try
            {
                BeginInvoke(() =>
                {
                    ExecuteSafely(() =>
                    {
                        if (!generations.IsCurrent(generation)) return;
                        action();
                    });
                });
            }
            catch (InvalidOperationException)
            {
            }
        }

        private void ClearLanding()
        {
            isLandingSequenceActive = false;
            isLandingVisible = false;
            isLandingAnimated = false;
            animationTimer.Stop();
            if (pendingPersistentCommand is not null)
            {
                OverlayRenderCommand pending = pendingPersistentCommand;
                pendingPersistentCommand = null;
                ShowOverlay(pending);
                return;
            }

            if (!restorePersistentAfterTransient)
            {
                HideOverlay();
                return;
            }

            RenderLayeredOverlay();
        }

        private void ClearScroll()
        {
            isScrollSequenceActive = false;
            isScrollVisible = false;
            isScrollAnimated = false;
            animationTimer.Stop();
            if (pendingPersistentCommand is not null)
            {
                OverlayRenderCommand pending = pendingPersistentCommand;
                pendingPersistentCommand = null;
                ShowOverlay(pending);
                return;
            }

            if (!restorePersistentAfterTransient)
            {
                HideOverlay();
                return;
            }

            RenderLayeredOverlay();
        }

        private void CancelPulseSchedule()
        {
            CancellationTokenSource? cancellation = pulseCancellation;
            pulseCancellation = null;
            if (cancellation is null) return;
            cancellation.Cancel();
            cancellation.Dispose();
        }

        private void ScheduleHide(int durationMs, long generation)
        {
            CancellationTokenSource cancellation = new();
            hideCancellation = cancellation;
            _ = HideAfterAsync(durationMs, generation, cancellation.Token);
        }

        private async Task HideAfterAsync(int durationMs, long generation, CancellationToken cancellationToken)
        {
            try
            {
                await Task.Delay(durationMs, cancellationToken).ConfigureAwait(false);
                if (IsDisposed || cancellationToken.IsCancellationRequested) return;
                try
                {
                    BeginInvoke(() =>
                    {
                        ExecuteSafely(() =>
                        {
                            if (generations.IsCurrent(generation)) HideOverlay();
                        });
                    });
                }
                catch (InvalidOperationException)
                {
                }
            }
            catch (OperationCanceledException)
            {
            }
        }

        private void CancelScheduledHide()
        {
            CancellationTokenSource? cancellation = hideCancellation;
            hideCancellation = null;
            if (cancellation is null) return;
            cancellation.Cancel();
            cancellation.Dispose();
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
            {
                CancelScheduledHide();
                CancelPulseSchedule();
                animationTimer.Dispose();
                horizontalCrosshair.Dispose();
                verticalCrosshair.Dispose();
            }

            base.Dispose(disposing);
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
