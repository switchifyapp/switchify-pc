using System.Text.Json;
using SwitchifyPc.Core.Settings;

namespace SwitchifyPc.Core.Input;

public sealed class MouseRepeatController
{
    private readonly DesktopCommandExecutor commandExecutor;
    private readonly IMouseRepeatSettingsStore settingsStore;
    private readonly Func<int, CancellationToken, Task> delay;
    private readonly object sync = new();
    private readonly Dictionary<string, ActiveRepeat> activeRepeats = new(StringComparer.Ordinal);

    public MouseRepeatController(
        DesktopCommandExecutor commandExecutor,
        IMouseRepeatSettingsStore settingsStore,
        Func<int, CancellationToken, Task>? delay = null)
    {
        this.commandExecutor = commandExecutor;
        this.settingsStore = settingsStore;
        this.delay = delay ?? ((milliseconds, cancellationToken) => Task.Delay(milliseconds, cancellationToken));
    }

    public async Task<CommandExecutionResult> StartAsync(string deviceId, JsonElement nestedCommand, CancellationToken cancellationToken = default)
    {
        MouseRepeatSettings settings = settingsStore.Load();
        if (!settings.Enabled)
        {
            await StopAsync(deviceId).ConfigureAwait(false);
            return CommandExecutionResult.Failure("repeat_disabled", "Mouse repeat is disabled.");
        }

        using JsonDocument commandDocument = JsonDocument.Parse(nestedCommand.GetRawText());
        CommandExecutionResult initial = await commandExecutor.ExecuteAsync(commandDocument.RootElement, cancellationToken).ConfigureAwait(false);
        if (!initial.Ok)
        {
            await StopAsync(deviceId).ConfigureAwait(false);
            return initial;
        }

        ActiveRepeat next = new(JsonDocument.Parse(nestedCommand.GetRawText()));
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

                await delay(settings.IntervalMs, repeat.Cancellation).ConfigureAwait(false);
                if (repeat.Cancellation.IsCancellationRequested) return;

                settings = settingsStore.Load();
                if (!settings.Enabled)
                {
                    await StopIfCurrentAsync(deviceId, repeat).ConfigureAwait(false);
                    return;
                }

                CommandExecutionResult result = await commandExecutor.ExecuteAsync(repeat.Command.RootElement, repeat.Cancellation).ConfigureAwait(false);
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

    private sealed class ActiveRepeat
    {
        private readonly CancellationTokenSource cancellation = new();

        public ActiveRepeat(JsonDocument command)
        {
            Command = command;
        }

        public JsonDocument Command { get; }

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
