using System.Text.Json;
using SwitchifyPc.Core.Input;
using SwitchifyPc.Core.Settings;
using SwitchifyPc.Protocol;

namespace SwitchifyPc.Tests;

public sealed class MouseRepeatControllerTests
{
    [Fact]
    public async Task MoveRepeatUsesMoveInterval()
    {
        FakeInputAdapter adapter = new();
        FakeMouseRepeatSettings settings = new(new MouseRepeatSettings(true, 1000, 100, 0));
        ManualDelay delay = new();
        MouseRepeatController controller = new(new DesktopCommandExecutor(adapter), settings, delay.DelayAsync);

        CommandExecutionResult result = await controller.StartAsync("device-1", Command("mouse.move", new { dx = 8, dy = 0 }));

        Assert.True(result.Ok);
        await delay.WaitForDelayCountAsync(1);
        Assert.Equal(1000, delay.Delays[0]);
        Assert.Equal(["moveMouseBy:8,0"], adapter.Calls);

        delay.CompleteNext();
        await WaitForCallCountAsync(adapter, 2);

        Assert.Equal(["moveMouseBy:8,0", "moveMouseBy:8,0"], adapter.Calls);
        await controller.StopAllAsync();
    }

    [Fact]
    public async Task ScrollRepeatUsesScrollInterval()
    {
        FakeInputAdapter adapter = new();
        FakeMouseRepeatSettings settings = new(new MouseRepeatSettings(true, 1000, 100, 0));
        ManualDelay delay = new();
        MouseRepeatController controller = new(new DesktopCommandExecutor(adapter), settings, delay.DelayAsync);

        CommandExecutionResult result = await controller.StartAsync("device-1", Command("mouse.scroll", new { dx = 0, dy = 5 }));

        Assert.True(result.Ok);
        await delay.WaitForDelayCountAsync(1);
        Assert.Equal(100, delay.Delays[0]);
        Assert.Equal(["scrollMouse:0,5"], adapter.Calls);

        delay.CompleteNext();
        await WaitForCallCountAsync(adapter, 2);

        Assert.Equal(["scrollMouse:0,5", "scrollMouse:0,5"], adapter.Calls);
        await controller.StopAllAsync();
    }

    [Fact]
    public async Task ScrollIntervalChangesApplyOnNextSchedulingCycle()
    {
        FakeInputAdapter adapter = new();
        FakeMouseRepeatSettings settings = new(new MouseRepeatSettings(true, 1000, 100, 0));
        ManualDelay delay = new();
        MouseRepeatController controller = new(new DesktopCommandExecutor(adapter), settings, delay.DelayAsync);

        await controller.StartAsync("device-1", Command("mouse.scroll", new { dx = 0, dy = 5 }));
        await delay.WaitForDelayCountAsync(1);

        settings.Save(new MouseRepeatSettings(true, 2000, 500, 0));
        delay.CompleteNext();
        await WaitForCallCountAsync(adapter, 2);
        await delay.WaitForDelayCountAsync(2);

        Assert.Equal([100, 500], delay.Delays);
        await controller.StopAllAsync();
    }

    [Fact]
    public async Task ReplacingMoveRepeatWithScrollRepeatUsesScrollInterval()
    {
        FakeInputAdapter adapter = new();
        FakeMouseRepeatSettings settings = new(new MouseRepeatSettings(true, 1000, 100, 0));
        ManualDelay delay = new();
        MouseRepeatController controller = new(new DesktopCommandExecutor(adapter), settings, delay.DelayAsync);

        await controller.StartAsync("device-1", Command("mouse.move", new { dx = 8, dy = 0 }));
        await delay.WaitForDelayCountAsync(1);
        await controller.StartAsync("device-1", Command("mouse.scroll", new { dx = 0, dy = 5 }));
        await delay.WaitForDelayCountAsync(2);

        Assert.Equal([1000, 100], delay.Delays);
        Assert.Equal(["moveMouseBy:8,0", "scrollMouse:0,5"], adapter.Calls);
        await controller.StopAllAsync();
    }

    [Fact]
    public async Task DisabledSettingStopsBeforeNextExecution()
    {
        FakeInputAdapter adapter = new();
        FakeMouseRepeatSettings settings = new(new MouseRepeatSettings(true, 100, 100, 0));
        ManualDelay delay = new();
        MouseRepeatController controller = new(new DesktopCommandExecutor(adapter), settings, delay.DelayAsync);

        await controller.StartAsync("device-1", Command("mouse.scroll", new { dx = 0, dy = 5 }));
        await delay.WaitForDelayCountAsync(1);
        settings.Save(new MouseRepeatSettings(false, 100, 100, 0));
        delay.CompleteNext();
        await WaitForInactiveAsync(controller, "device-1");

        Assert.Equal(["scrollMouse:0,5"], adapter.Calls);
    }

    [Fact]
    public async Task StopIsIdempotent()
    {
        FakeInputAdapter adapter = new();
        FakeMouseRepeatSettings settings = new(new MouseRepeatSettings(true, 100, 100, 0));
        ManualDelay delay = new();
        MouseRepeatController controller = new(new DesktopCommandExecutor(adapter), settings, delay.DelayAsync);

        await controller.StartAsync("device-1", Command("mouse.scroll", new { dx = 0, dy = 5 }));

        await controller.StopAsync("device-1");
        await controller.StopAsync("device-1");

        Assert.False(controller.IsActive("device-1"));
    }

    [Fact]
    public async Task MoveRepeatFeedbackStartsAfterSuccessAndEndsOnStop()
    {
        FakeInputAdapter adapter = new();
        FakeMouseRepeatSettings settings = new(new MouseRepeatSettings(true, 100, 100, 1000));
        ManualDelay delay = new();
        FakeRepeatFeedback feedback = new();
        MouseRepeatController controller = new(
            new DesktopCommandExecutor(adapter),
            settings,
            delay.DelayAsync,
            feedbackNotifier: feedback);

        CommandExecutionResult result = await controller.StartAsync(
            "device-1",
            Command("mouse.move", new { dx = 8, dy = -4 }));

        Assert.True(result.Ok);
        MouseRepeatFeedback started = Assert.Single(feedback.Started);
        Assert.Equal(MouseRepeatFeedbackKind.Move, started.Kind);
        Assert.Equal(8, started.Dx);
        Assert.Equal(-4, started.Dy);
        Assert.Equal(1000, started.AccelerationDurationMs);

        await controller.StopAsync("device-1");

        Assert.Equal(started.GenerationId, Assert.Single(feedback.Ended));
    }

    [Fact]
    public async Task ScrollRepeatFeedbackKeepsDirectionWithoutAcceleration()
    {
        FakeRepeatFeedback feedback = new();
        MouseRepeatController controller = new(
            new DesktopCommandExecutor(new FakeInputAdapter()),
            new FakeMouseRepeatSettings(new MouseRepeatSettings(true, 100, 100, 2000)),
            feedbackNotifier: feedback);

        await controller.StartAsync(
            "device-1",
            Command("mouse.scroll", new { dx = 3, dy = -5 }));

        MouseRepeatFeedback started = Assert.Single(feedback.Started);
        Assert.Equal(MouseRepeatFeedbackKind.Scroll, started.Kind);
        Assert.Equal(3, started.Dx);
        Assert.Equal(-5, started.Dy);
        Assert.Equal(0, started.AccelerationDurationMs);
        await controller.StopAllAsync();
    }

    [Fact]
    public async Task ReplacingRepeatEndsOnlyTheSupersededGeneration()
    {
        FakeRepeatFeedback feedback = new();
        MouseRepeatController controller = new(
            new DesktopCommandExecutor(new FakeInputAdapter()),
            new FakeMouseRepeatSettings(new MouseRepeatSettings(true, 100, 100, 1000)),
            feedbackNotifier: feedback);

        await controller.StartAsync("device-1", Command("mouse.move", new { dx = 8, dy = 0 }));
        Guid first = Assert.Single(feedback.Started).GenerationId;
        await controller.StartAsync("device-1", Command("mouse.scroll", new { dx = 0, dy = 5 }));
        Guid second = feedback.Started[1].GenerationId;

        Assert.Equal(first, Assert.Single(feedback.Ended));
        Assert.NotEqual(first, second);

        await controller.StopAllAsync();

        Assert.Equal([first, second], feedback.Ended);
    }

    [Fact]
    public async Task RepeatExecutionFailureEndsFeedback()
    {
        FakeInputAdapter adapter = new();
        ManualDelay delay = new();
        FakeRepeatFeedback feedback = new();
        MouseRepeatController controller = new(
            new DesktopCommandExecutor(adapter),
            new FakeMouseRepeatSettings(new MouseRepeatSettings(true, 100, 100, 1000)),
            delay.DelayAsync,
            feedbackNotifier: feedback);

        await controller.StartAsync("device-1", Command("mouse.move", new { dx = 8, dy = 0 }));
        await delay.WaitForDelayCountAsync(1);
        adapter.FailMoves = true;
        delay.CompleteNext();
        await WaitForInactiveAsync(controller, "device-1");

        Assert.Equal(feedback.Started[0].GenerationId, Assert.Single(feedback.Ended));
    }

    [Fact]
    public async Task InitialExecutionFailureDoesNotStartFeedback()
    {
        FakeInputAdapter adapter = new() { FailMoves = true };
        FakeRepeatFeedback feedback = new();
        MouseRepeatController controller = new(
            new DesktopCommandExecutor(adapter),
            new FakeMouseRepeatSettings(new MouseRepeatSettings(true, 100, 100, 1000)),
            feedbackNotifier: feedback);

        CommandExecutionResult result = await controller.StartAsync(
            "device-1",
            Command("mouse.move", new { dx = 8, dy = 0 }));

        Assert.False(result.Ok);
        Assert.Empty(feedback.Started);
        Assert.Empty(feedback.Ended);
    }

    [Fact]
    public async Task FeedbackFailureDoesNotFailRepeatExecutionOrStop()
    {
        FakeRepeatFeedback feedback = new() { ThrowOnNotification = true };
        MouseRepeatController controller = new(
            new DesktopCommandExecutor(new FakeInputAdapter()),
            new FakeMouseRepeatSettings(new MouseRepeatSettings(true, 100, 100, 1000)),
            feedbackNotifier: feedback);

        CommandExecutionResult result = await controller.StartAsync(
            "device-1",
            Command("mouse.move", new { dx = 8, dy = 0 }));

        Assert.True(result.Ok);
        await controller.StopAsync("device-1");
        Assert.False(controller.IsActive("device-1"));
    }

    [Fact]
    public async Task MoveRepeatAcceleratesFromInitialScaleToFullDistance()
    {
        FakeInputAdapter adapter = new();
        FakeMouseRepeatSettings settings = new(new MouseRepeatSettings(true, 100, 100, 1000));
        ManualDelay delay = new();
        ManualTimeProvider time = new();
        MouseRepeatController controller = new(new DesktopCommandExecutor(adapter), settings, delay.DelayAsync, time);

        await controller.StartAsync("device-1", Command("mouse.move", new { dx = 8, dy = -4 }));

        Assert.Equal((2, -1), adapter.Moves[0]);
        await delay.WaitForDelayCountAsync(1);
        time.Advance(TimeSpan.FromMilliseconds(500));
        delay.CompleteNext();
        await WaitForCallCountAsync(adapter, 2);

        Assert.Equal((5, -2.5), adapter.Moves[1]);
        await delay.WaitForDelayCountAsync(2);
        time.Advance(TimeSpan.FromMilliseconds(500));
        delay.CompleteNext();
        await WaitForCallCountAsync(adapter, 3);

        Assert.Equal((8, -4), adapter.Moves[2]);
        await controller.StopAllAsync();
    }

    [Fact]
    public async Task ReplacingMoveRepeatRestartsAcceleration()
    {
        FakeInputAdapter adapter = new();
        FakeMouseRepeatSettings settings = new(new MouseRepeatSettings(true, 100, 100, 1000));
        ManualDelay delay = new();
        ManualTimeProvider time = new();
        MouseRepeatController controller = new(new DesktopCommandExecutor(adapter), settings, delay.DelayAsync, time);

        await controller.StartAsync("device-1", Command("mouse.move", new { dx = 8, dy = 0 }));
        await delay.WaitForDelayCountAsync(1);
        time.Advance(TimeSpan.FromMilliseconds(1000));
        delay.CompleteNext();
        await WaitForCallCountAsync(adapter, 2);
        await controller.StartAsync("device-1", Command("mouse.move", new { dx = -8, dy = 0 }));

        Assert.Equal([(2, 0), (8, 0), (-2, 0)], adapter.Moves);
        await controller.StopAllAsync();
    }

    [Fact]
    public async Task ActiveRepeatKeepsAccelerationDurationFromStart()
    {
        FakeInputAdapter adapter = new();
        FakeMouseRepeatSettings settings = new(new MouseRepeatSettings(true, 100, 100, 1000));
        ManualDelay delay = new();
        ManualTimeProvider time = new();
        MouseRepeatController controller = new(new DesktopCommandExecutor(adapter), settings, delay.DelayAsync, time);

        await controller.StartAsync("device-1", Command("mouse.move", new { dx = 8, dy = 0 }));
        await delay.WaitForDelayCountAsync(1);
        settings.Save(new MouseRepeatSettings(true, 100, 100, 0));
        time.Advance(TimeSpan.FromMilliseconds(500));
        delay.CompleteNext();
        await WaitForCallCountAsync(adapter, 2);

        Assert.Equal((5, 0), adapter.Moves[1]);
        await controller.StopAllAsync();
    }

    private static JsonElement Command(string type, object payload)
    {
        Dictionary<string, object?> command = new(StringComparer.Ordinal)
        {
            ["version"] = ProtocolConstants.ProtocolVersion,
            ["id"] = "request-1",
            ["deviceId"] = "android-1",
            ["timestamp"] = 1,
            ["type"] = type,
            ["payload"] = payload,
            ["auth"] = "proof"
        };

        return JsonSerializer.SerializeToElement(command);
    }

    private static async Task WaitForCallCountAsync(FakeInputAdapter adapter, int count)
    {
        using CancellationTokenSource timeout = new(TimeSpan.FromSeconds(5));
        while (adapter.Calls.Count < count)
        {
            timeout.Token.ThrowIfCancellationRequested();
            await Task.Delay(10, timeout.Token);
        }
    }

    private static async Task WaitForInactiveAsync(MouseRepeatController controller, string deviceId)
    {
        using CancellationTokenSource timeout = new(TimeSpan.FromSeconds(5));
        while (controller.IsActive(deviceId))
        {
            timeout.Token.ThrowIfCancellationRequested();
            await Task.Delay(10, timeout.Token);
        }
    }

    private sealed class ManualDelay
    {
        private readonly object sync = new();
        private readonly Queue<TaskCompletionSource> pending = new();
        private readonly List<int> delays = [];

        public IReadOnlyList<int> Delays
        {
            get
            {
                lock (sync)
                {
                    return delays.ToArray();
                }
            }
        }

        public Task DelayAsync(int milliseconds, CancellationToken cancellationToken)
        {
            TaskCompletionSource completion = new(TaskCreationOptions.RunContinuationsAsynchronously);
            lock (sync)
            {
                delays.Add(milliseconds);
                pending.Enqueue(completion);
                Monitor.PulseAll(sync);
            }

            cancellationToken.Register(() => completion.TrySetCanceled(cancellationToken));
            return completion.Task;
        }

        public void CompleteNext()
        {
            TaskCompletionSource completion;
            lock (sync)
            {
                completion = pending.Dequeue();
            }

            completion.SetResult();
        }

        public Task WaitForDelayCountAsync(int count)
        {
            return Task.Run(() =>
            {
                DateTimeOffset deadline = DateTimeOffset.UtcNow.AddSeconds(5);
                lock (sync)
                {
                    while (delays.Count < count)
                    {
                        TimeSpan remaining = deadline - DateTimeOffset.UtcNow;
                        if (remaining <= TimeSpan.Zero)
                        {
                            throw new TimeoutException("Timed out waiting for repeat delay.");
                        }

                        Monitor.Wait(sync, remaining);
                    }
                }
            });
        }
    }

    private sealed class FakeMouseRepeatSettings(MouseRepeatSettings settings) : IMouseRepeatSettingsStore
    {
        public MouseRepeatSettings Load() => settings;

        public MouseRepeatSettings Save(MouseRepeatSettings next)
        {
            settings = MouseRepeatSettingsModel.Normalize(next);
            return settings;
        }
    }

    private sealed class FakeInputAdapter : IDesktopInputAdapter
    {
        public List<string> Calls { get; } = [];

        public List<(double Dx, double Dy)> Moves { get; } = [];

        public bool FailMoves { get; set; }

        public Task MoveMouseByAsync(double dx, double dy, CancellationToken cancellationToken = default)
        {
            if (FailMoves) throw new DesktopInputException("adapter_failure", "Move failed.");
            Moves.Add((dx, dy));
            Calls.Add($"moveMouseBy:{dx},{dy}");
            return Task.CompletedTask;
        }

        public Task SetMouseButtonDownAsync(string button, bool down, CancellationToken cancellationToken = default) => Task.CompletedTask;

        public Task ClickMouseAsync(string button, CancellationToken cancellationToken = default) => Task.CompletedTask;

        public Task DoubleClickMouseAsync(string button, CancellationToken cancellationToken = default) => Task.CompletedTask;

        public Task ScrollMouseAsync(double dx, double dy, CancellationToken cancellationToken = default)
        {
            Calls.Add($"scrollMouse:{dx},{dy}");
            return Task.CompletedTask;
        }

        public Task PressKeyAsync(string key, CancellationToken cancellationToken = default) => Task.CompletedTask;

        public Task SetKeyDownAsync(string key, bool down, CancellationToken cancellationToken = default) => Task.CompletedTask;

        public Task PressShortcutAsync(IReadOnlyList<string> keys, CancellationToken cancellationToken = default) => Task.CompletedTask;

        public Task TypeTextAsync(string text, CancellationToken cancellationToken = default) => Task.CompletedTask;

        public Task TypeCharacterAsync(string text, CancellationToken cancellationToken = default) => Task.CompletedTask;

        public Task MediaControlAsync(string action, CancellationToken cancellationToken = default) => Task.CompletedTask;

        public Task ControlWindowAsync(string action, CancellationToken cancellationToken = default) => Task.CompletedTask;
    }

    private sealed class FakeRepeatFeedback : IMouseRepeatFeedbackNotifier
    {
        public List<MouseRepeatFeedback> Started { get; } = [];
        public List<Guid> Ended { get; } = [];

        public bool ThrowOnNotification { get; set; }

        public void BeginRepeat(MouseRepeatFeedback feedback)
        {
            if (ThrowOnNotification) throw new InvalidOperationException("Feedback failed.");
            Started.Add(feedback);
        }

        public void EndRepeat(Guid generationId)
        {
            if (ThrowOnNotification) throw new InvalidOperationException("Feedback failed.");
            Ended.Add(generationId);
        }
    }

    private sealed class ManualTimeProvider : TimeProvider
    {
        private long timestamp;

        public override long TimestampFrequency => 1000;

        public override long GetTimestamp() => timestamp;

        public void Advance(TimeSpan elapsed)
        {
            timestamp += (long)elapsed.TotalMilliseconds;
        }
    }
}
