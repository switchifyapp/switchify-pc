using System.Text.Json;
using SwitchifyPc.Core.Settings;

namespace SwitchifyPc.Core.Input;

public sealed class MouseRepeatController
{
    private readonly DesktopCommandExecutor commandExecutor;
    private readonly IMouseRepeatSettingsStore settingsStore;
    private readonly Func<int, CancellationToken, Task> delay;
    private readonly TimeProvider timeProvider;
    private readonly object sync = new();
    private readonly Dictionary<string, ActiveRepeat> activeRepeats = new(StringComparer.Ordinal);

    public MouseRepeatController(
        DesktopCommandExecutor commandExecutor,
        IMouseRepeatSettingsStore settingsStore,
        Func<int, CancellationToken, Task>? delay = null,
        TimeProvider? timeProvider = null)
    {
        this.commandExecutor = commandExecutor;
        this.settingsStore = settingsStore;
        this.delay = delay ?? ((milliseconds, cancellationToken) => Task.Delay(milliseconds, cancellationToken));
        this.timeProvider = timeProvider ?? TimeProvider.System;
    }

    public async Task<CommandExecutionResult> StartAsync(string deviceId, JsonElement nestedCommand, CancellationToken cancellationToken = default)
    {
        MouseRepeatSettings settings = settingsStore.Load();
        if (!settings.Enabled)
        {
            await StopAsync(deviceId).ConfigureAwait(false);
            return CommandExecutionResult.Failure("repeat_disabled", "Mouse repeat is disabled.");
        }

        ActiveRepeat next = new(
            JsonDocument.Parse(nestedCommand.GetRawText()),
            settings.AccelerationDurationMs,
            timeProvider.GetTimestamp());
        CommandExecutionResult initial = await ExecuteAsync(next, cancellationToken, TimeSpan.Zero).ConfigureAwait(false);
        if (!initial.Ok)
        {
            next.Dispose();
            await StopAsync(deviceId).ConfigureAwait(false);
            return initial;
        }

        ActiveRepeat? previous;
        lock (sync)
        {
            activeRepeats.Remove(deviceId, out previous);
            activeRepeats[deviceId] = next;
        }

        previous?.Stop();
        _ = RunAsync(deviceId, next);
        return CommandExecutionResult.Success;
    }

    public Task StopAsync(string deviceId)
    {
        ActiveRepeat? repeat;
        lock (sync)
        {
            activeRepeats.Remove(deviceId, out repeat);
        }

        repeat?.Stop();
        return Task.CompletedTask;
    }

    public Task StopAllAsync()
    {
        ActiveRepeat[] repeats;
        lock (sync)
        {
            repeats = activeRepeats.Values.ToArray();
            activeRepeats.Clear();
        }

        foreach (ActiveRepeat repeat in repeats)
        {
            repeat.Stop();
        }

        return Task.CompletedTask;
    }

    public bool IsActive(string deviceId)
    {
        lock (sync)
        {
            return activeRepeats.ContainsKey(deviceId);
        }
    }

    private async Task RunAsync(string deviceId, ActiveRepeat repeat)
    {
        try
        {
            while (!repeat.Cancellation.IsCancellationRequested)
            {
                MouseRepeatSettings settings = settingsStore.Load();
                if (!settings.Enabled)
                {
                    await StopIfCurrentAsync(deviceId, repeat).ConfigureAwait(false);
                    return;
                }

                await delay(IntervalFor(settings, repeat.Command.RootElement), repeat.Cancellation).ConfigureAwait(false);
                if (repeat.Cancellation.IsCancellationRequested) return;

                settings = settingsStore.Load();
                if (!settings.Enabled)
                {
                    await StopIfCurrentAsync(deviceId, repeat).ConfigureAwait(false);
                    return;
                }

                CommandExecutionResult result = await ExecuteAsync(repeat, repeat.Cancellation).ConfigureAwait(false);
                if (!result.Ok)
                {
                    await StopIfCurrentAsync(deviceId, repeat).ConfigureAwait(false);
                    return;
                }
            }
        }
        catch (OperationCanceledException)
        {
        }
        finally
        {
            repeat.Dispose();
        }
    }

    private Task StopIfCurrentAsync(string deviceId, ActiveRepeat repeat)
    {
        bool removed = false;
        lock (sync)
        {
            if (activeRepeats.TryGetValue(deviceId, out ActiveRepeat? current) && ReferenceEquals(current, repeat))
            {
                activeRepeats.Remove(deviceId);
                removed = true;
            }
        }

        if (removed)
        {
            repeat.Stop();
        }

        return Task.CompletedTask;
    }

    private static int IntervalFor(MouseRepeatSettings settings, JsonElement command)
    {
        return command.GetProperty("type").GetString() == "mouse.scroll"
            ? settings.ScrollIntervalMs
            : settings.MoveIntervalMs;
    }

    private Task<CommandExecutionResult> ExecuteAsync(
        ActiveRepeat repeat,
        CancellationToken cancellationToken,
        TimeSpan? elapsed = null)
    {
        JsonElement command = repeat.Command.RootElement;
        if (command.GetProperty("type").GetString() != "mouse.move")
        {
            return commandExecutor.ExecuteAsync(command, cancellationToken);
        }

        JsonElement payload = command.GetProperty("payload");
        double scale = MouseRepeatSettingsModel.AccelerationScale(
            repeat.AccelerationDurationMs,
            elapsed ?? timeProvider.GetElapsedTime(repeat.StartTimestamp));
        JsonElement scaledCommand = JsonSerializer.SerializeToElement(new
        {
            type = "mouse.move",
            payload = new
            {
                dx = payload.GetProperty("dx").GetDouble() * scale,
                dy = payload.GetProperty("dy").GetDouble() * scale
            }
        });
        return commandExecutor.ExecuteAsync(scaledCommand, cancellationToken);
    }

    private sealed class ActiveRepeat
    {
        private readonly CancellationTokenSource cancellation = new();

        public ActiveRepeat(JsonDocument command, int accelerationDurationMs, long startTimestamp)
        {
            Command = command;
            AccelerationDurationMs = accelerationDurationMs;
            StartTimestamp = startTimestamp;
        }

        public JsonDocument Command { get; }

        public int AccelerationDurationMs { get; }

        public long StartTimestamp { get; }

        public CancellationToken Cancellation => cancellation.Token;

        public void Stop()
        {
            cancellation.Cancel();
        }

        public void Dispose()
        {
            cancellation.Dispose();
            Command.Dispose();
        }
    }
}
