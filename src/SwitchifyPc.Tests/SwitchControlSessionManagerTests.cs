using SwitchifyPc.Core.SwitchControl;

namespace SwitchifyPc.Tests;

public sealed class SwitchControlSessionManagerTests
{
    [Fact]
    public async Task StaleSequencesAreIdempotent()
    {
        SwitchControlProfile profile = SwitchControlProfiles.BuiltIns[1];
        var output = new RecordingOutputSession();
        var manager = Manager(profile, output);
        string sessionId = Guid.NewGuid().ToString();
        await manager.StartAsync("device", sessionId, profile.Id, profile.Version);

        await manager.ApplyEdgeAsync("device", sessionId, 2, 1, true);
        await manager.ApplyEdgeAsync("device", sessionId, 1, 1, false);

        Assert.Equal(["1:down"], output.Events);
    }

    [Fact]
    public async Task ReplacementStopsOldOutputBeforeStartingNewSession()
    {
        SwitchControlProfile profile = SwitchControlProfiles.BuiltIns[1];
        var first = new RecordingOutputSession();
        var second = new RecordingOutputSession();
        var factory = new QueueOutputFactory(first, second);
        var manager = new SwitchControlSessionManager(new StaticProfileStore(profile), factory);

        await manager.StartAsync("one", Guid.NewGuid().ToString(), profile.Id, profile.Version);
        await manager.StartAsync("two", Guid.NewGuid().ToString(), profile.Id, profile.Version);

        Assert.Equal(1, first.StopCount);
        Assert.Equal(0, second.StopCount);
    }

    [Fact]
    public async Task OutputFailureFaultsSessionAndDoesNotAdvanceSequence()
    {
        SwitchControlProfile profile = SwitchControlProfiles.BuiltIns[1];
        var output = new RecordingOutputSession { Fail = true };
        var manager = Manager(profile, output);
        string sessionId = Guid.NewGuid().ToString();
        await manager.StartAsync("device", sessionId, profile.Id, profile.Version);

        SwitchSessionResult first = await manager.ApplyEdgeAsync("device", sessionId, 1, 1, true);
        output.Fail = false;
        SwitchSessionResult second = await manager.ApplyEdgeAsync("device", sessionId, 1, 1, true);

        Assert.Equal("output_failure", first.Code);
        Assert.Equal("output_failure", second.Code);
        Assert.Empty(output.Events);
    }

    private static SwitchControlSessionManager Manager(
        SwitchControlProfile profile,
        RecordingOutputSession output) =>
        new(new StaticProfileStore(profile), new QueueOutputFactory(output));

    private sealed class StaticProfileStore(params SwitchControlProfile[] profiles) : ISwitchControlProfileStore
    {
        public IReadOnlyList<SwitchControlProfile> Load() => profiles;
        public IReadOnlyList<SwitchControlProfile> Save(IReadOnlyList<SwitchControlProfile> customProfiles) => customProfiles;
    }

    private sealed class QueueOutputFactory(params RecordingOutputSession[] outputs) : ISwitchOutputSessionFactory
    {
        private readonly Queue<RecordingOutputSession> queue = new(outputs);
        public ISwitchOutputSession Create(SwitchControlProfile profile) => queue.Dequeue();
    }

    private sealed class RecordingOutputSession : ISwitchOutputSession
    {
        public List<string> Events { get; } = [];
        public int StopCount { get; private set; }
        public bool Fail { get; set; }

        public Task ApplyEdgeAsync(int switchId, bool pressed, CancellationToken token)
        {
            if (Fail) throw new InvalidOperationException();
            Events.Add($"{switchId}:{(pressed ? "down" : "up")}");
            return Task.CompletedTask;
        }

        public Task SynchronizeAsync(IReadOnlySet<int> pressedSwitchIds, CancellationToken token) => Task.CompletedTask;

        public Task StopAsync(CancellationToken token)
        {
            StopCount++;
            return Task.CompletedTask;
        }
    }
}
