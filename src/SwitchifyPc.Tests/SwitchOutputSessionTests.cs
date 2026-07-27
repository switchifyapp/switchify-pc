using SwitchifyPc.Core.Input;
using SwitchifyPc.Core.SwitchControl;

namespace SwitchifyPc.Tests;

public sealed class SwitchOutputSessionTests
{
    [Fact]
    public async Task SharedStatefulOutputUsesReferenceCounting()
    {
        var input = new RecordingInputAdapter();
        var session = new MappedDesktopSwitchOutputSession(input,
        [
            new(1, SwitchBindingType.Key, "Space"),
            new(2, SwitchBindingType.Key, "Space")
        ]);

        await session.ApplyEdgeAsync(1, true, default);
        await session.ApplyEdgeAsync(2, true, default);
        await session.ApplyEdgeAsync(1, false, default);
        await session.ApplyEdgeAsync(2, false, default);

        Assert.Equal(["key:Space:down", "key:Space:up"], input.Events);
    }

    [Fact]
    public async Task PulseRunsOnceAndNeverFromSnapshot()
    {
        var input = new RecordingInputAdapter();
        var session = new MappedDesktopSwitchOutputSession(input,
        [
            new(1, SwitchBindingType.MouseClick, "left")
        ]);

        await session.ApplyEdgeAsync(1, true, default);
        await session.ApplyEdgeAsync(1, true, default);
        await session.SynchronizeAsync(new HashSet<int> { 1 }, default);

        Assert.Equal(["click:left"], input.Events);
    }

    [Fact]
    public async Task SnapshotReleasesBeforePresses()
    {
        var input = new RecordingInputAdapter();
        var session = new MappedDesktopSwitchOutputSession(input,
        [
            new(1, SwitchBindingType.Key, "Space"),
            new(2, SwitchBindingType.Key, "Enter")
        ]);
        await session.ApplyEdgeAsync(1, true, default);

        await session.SynchronizeAsync(new HashSet<int> { 2 }, default);

        Assert.Equal(["key:Space:down", "key:Space:up", "key:Enter:down"], input.Events);
    }

    [Fact]
    public async Task ShortcutDoesNotReleaseAlreadyHeldKey()
    {
        var input = new RecordingInputAdapter();
        var session = new MappedDesktopSwitchOutputSession(input,
        [
            new(1, SwitchBindingType.Key, "Ctrl"),
            new(2, SwitchBindingType.Shortcut, Keys: ["Ctrl", "C"])
        ]);

        await session.ApplyEdgeAsync(1, true, default);
        await session.ApplyEdgeAsync(2, true, default);

        Assert.Equal(["key:Ctrl:down", "key:C:down", "key:C:up"], input.Events);
    }

    [Fact]
    public async Task FailedDownDoesNotMutateTrackedOutput()
    {
        var input = new RecordingInputAdapter { FailNext = true };
        var session = new MappedDesktopSwitchOutputSession(input,
        [
            new(1, SwitchBindingType.Key, "Space")
        ]);

        await Assert.ThrowsAsync<DesktopInputException>(() => session.ApplyEdgeAsync(1, true, default));
        await session.ApplyEdgeAsync(1, true, default);

        Assert.Equal(["key:Space:down"], input.Events);
    }

    private sealed class RecordingInputAdapter : IDesktopInputAdapter
    {
        public List<string> Events { get; } = [];
        public bool FailNext { get; set; }

        public Task SetKeyDownAsync(string key, bool down, CancellationToken cancellationToken = default) =>
            Record($"key:{key}:{(down ? "down" : "up")}");
        public Task SetMouseButtonDownAsync(string button, bool down, CancellationToken cancellationToken = default) =>
            Record($"mouse:{button}:{(down ? "down" : "up")}");
        public Task ClickMouseAsync(string button, CancellationToken cancellationToken = default) => Record($"click:{button}");
        public Task DoubleClickMouseAsync(string button, CancellationToken cancellationToken = default) => Record($"double:{button}");
        public Task ScrollMouseAsync(double dx, double dy, CancellationToken cancellationToken = default) => Record($"scroll:{dx}:{dy}");
        public Task MediaControlAsync(string action, CancellationToken cancellationToken = default) => Record($"media:{action}");
        public Task MoveMouseByAsync(double dx, double dy, CancellationToken cancellationToken = default) => Task.CompletedTask;
        public Task PressKeyAsync(string key, CancellationToken cancellationToken = default) => Task.CompletedTask;
        public Task PressShortcutAsync(IReadOnlyList<string> keys, CancellationToken cancellationToken = default) => Task.CompletedTask;
        public Task TypeTextAsync(string text, CancellationToken cancellationToken = default) => Task.CompletedTask;
        public Task TypeCharacterAsync(string text, CancellationToken cancellationToken = default) => Task.CompletedTask;
        public Task ControlWindowAsync(string action, CancellationToken cancellationToken = default) => Task.CompletedTask;

        private Task Record(string value)
        {
            if (FailNext)
            {
                FailNext = false;
                throw new DesktopInputException("adapter_failure", "Failed.");
            }
            Events.Add(value);
            return Task.CompletedTask;
        }
    }
}
