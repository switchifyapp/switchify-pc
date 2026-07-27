using SwitchifyPc.Core.Input;

namespace SwitchifyPc.Core.SwitchControl;

public sealed record SwitchSessionResult(bool Ok, string? Code = null, string? Message = null)
{
    public static SwitchSessionResult Success { get; } = new(true);
    public static SwitchSessionResult Failure(string code, string message) => new(false, code, message);
}

public sealed class SwitchControlSessionManager
{
    private readonly ISwitchControlProfileStore profiles;
    private readonly ISwitchOutputSessionFactory outputs;
    private readonly SemaphoreSlim gate = new(1, 1);
    private ActiveSession? active;
    private long catalogRevision = 1;

    public event Action<string?>? ActiveProfileChanged;

    public SwitchControlSessionManager(
        ISwitchControlProfileStore profiles,
        ISwitchOutputSessionFactory outputs)
    {
        this.profiles = profiles;
        this.outputs = outputs;
    }

    public bool IsActive => active is not null;
    public string? ActiveProfileId => active?.Profile.Id;

    public SwitchControlProfileCatalog GetCatalog()
    {
        IReadOnlyList<SwitchControlProfile> loaded = profiles.Load();
        return new SwitchControlProfileCatalog(
            checked((int)catalogRevision),
            loaded.Select(SwitchControlProfiles.Summarize).ToArray());
    }

    public async Task<SwitchSessionResult> StartAsync(
        string deviceId,
        string sessionId,
        string profileId,
        int profileVersion,
        CancellationToken token = default)
    {
        if (!Guid.TryParse(sessionId, out _))
        {
            return SwitchSessionResult.Failure("invalid_payload", "Session ID must be a UUID.");
        }

        SwitchControlProfile? profile = profiles.Load().FirstOrDefault(candidate => candidate.Id == profileId);
        if (profile is null || profile.Version != profileVersion)
        {
            return SwitchSessionResult.Failure("profile_changed", "The selected profile changed or is no longer available.");
        }

        await gate.WaitAsync(token).ConfigureAwait(false);
        try
        {
            try
            {
                await StopLockedAsync(token).ConfigureAwait(false);
            }
            catch
            {
                return SwitchSessionResult.Failure(
                    "output_failure",
                    "The previous switch output could not be released.");
            }
            ISwitchOutputSession output;
            try
            {
                output = outputs.Create(profile);
            }
            catch (DesktopInputException error)
            {
                return SwitchSessionResult.Failure(error.Code, error.Message);
            }

            active = new ActiveSession(deviceId, sessionId, profile, output);
            ActiveProfileChanged?.Invoke(profile.Name);
            return SwitchSessionResult.Success;
        }
        finally
        {
            gate.Release();
        }
    }

    public Task<SwitchSessionResult> ApplyEdgeAsync(
        string deviceId,
        string sessionId,
        long sequence,
        int switchId,
        bool pressed,
        CancellationToken token = default) =>
        ExecuteSequencedAsync(deviceId, sessionId, sequence, output => output.ApplyEdgeAsync(switchId, pressed, token), token);

    public Task<SwitchSessionResult> SynchronizeAsync(
        string deviceId,
        string sessionId,
        long sequence,
        IReadOnlySet<int> pressedSwitchIds,
        CancellationToken token = default) =>
        ExecuteSequencedAsync(deviceId, sessionId, sequence, output => output.SynchronizeAsync(pressedSwitchIds, token), token);

    public async Task<SwitchSessionResult> StopAsync(
        string deviceId,
        string sessionId,
        long sequence,
        CancellationToken token = default)
    {
        await gate.WaitAsync(token).ConfigureAwait(false);
        try
        {
            if (active is null || active.DeviceId != deviceId || active.SessionId != sessionId)
            {
                return SwitchSessionResult.Success;
            }
            if (sequence <= active.LastSequence)
            {
                return SwitchSessionResult.Success;
            }

            await StopLockedAsync(token).ConfigureAwait(false);
            return SwitchSessionResult.Success;
        }
        catch
        {
            return SwitchSessionResult.Failure("output_failure", "The active switch output could not be released.");
        }
        finally
        {
            gate.Release();
        }
    }

    public async Task<SwitchSessionResult> ApplyLegacyGridEdgeAsync(
        string deviceId,
        string? sessionId,
        long? sequence,
        int switchId,
        bool pressed,
        CancellationToken token = default)
    {
        await gate.WaitAsync(token).ConfigureAwait(false);
        try
        {
            SwitchSessionResult ready = await EnsureLegacyGridSessionLockedAsync(deviceId, sessionId, token).ConfigureAwait(false);
            if (!ready.Ok) return ready;
            long effectiveSequence = sequence ?? active!.LastSequence + 1;
            return await ExecuteActiveLockedAsync(effectiveSequence, output =>
                output.ApplyEdgeAsync(switchId, pressed, token)).ConfigureAwait(false);
        }
        finally
        {
            gate.Release();
        }
    }

    public async Task<SwitchSessionResult> SynchronizeLegacyGridAsync(
        string deviceId,
        string sessionId,
        long sequence,
        IReadOnlySet<int> pressedSwitchIds,
        CancellationToken token = default)
    {
        await gate.WaitAsync(token).ConfigureAwait(false);
        try
        {
            SwitchSessionResult ready = await EnsureLegacyGridSessionLockedAsync(deviceId, sessionId, token).ConfigureAwait(false);
            return ready.Ok
                ? await ExecuteActiveLockedAsync(sequence, output =>
                    output.SynchronizeAsync(pressedSwitchIds, token)).ConfigureAwait(false)
                : ready;
        }
        finally
        {
            gate.Release();
        }
    }

    public async Task StopForDeviceAsync(string deviceId, CancellationToken token = default)
    {
        await gate.WaitAsync(token).ConfigureAwait(false);
        try
        {
            if (active?.DeviceId == deviceId)
            {
                await StopLockedAsync(token).ConfigureAwait(false);
            }
        }
        finally
        {
            gate.Release();
        }
    }

    public async Task StopAllAsync(CancellationToken token = default)
    {
        await gate.WaitAsync(token).ConfigureAwait(false);
        try
        {
            await StopLockedAsync(token).ConfigureAwait(false);
        }
        finally
        {
            gate.Release();
        }
    }

    private async Task<SwitchSessionResult> ExecuteSequencedAsync(
        string deviceId,
        string sessionId,
        long sequence,
        Func<ISwitchOutputSession, Task> operation,
        CancellationToken token)
    {
        if (sequence < 1)
        {
            return SwitchSessionResult.Failure("invalid_payload", "Sequence must be positive.");
        }

        await gate.WaitAsync(token).ConfigureAwait(false);
        try
        {
            if (active is null || active.DeviceId != deviceId || active.SessionId != sessionId)
            {
                return SwitchSessionResult.Failure("session_not_active", "The switch-control session is not active.");
            }
            return await ExecuteActiveLockedAsync(sequence, operation).ConfigureAwait(false);
        }
        finally
        {
            gate.Release();
        }
    }

    private async Task<SwitchSessionResult> EnsureLegacyGridSessionLockedAsync(
        string deviceId,
        string? sessionId,
        CancellationToken token)
    {
        string effectiveSessionId = sessionId ?? $"legacy:{deviceId}";
        if (active is not null &&
            active.DeviceId == deviceId &&
            active.SessionId == effectiveSessionId &&
            active.Profile.Id == SwitchControlProfiles.Grid3Id)
        {
            return SwitchSessionResult.Success;
        }

        try
        {
            await StopLockedAsync(token).ConfigureAwait(false);
        }
        catch
        {
            return SwitchSessionResult.Failure(
                "output_failure",
                "The previous switch output could not be released.");
        }
        SwitchControlProfile profile = SwitchControlProfiles.BuiltIns[0];
        try
        {
            active = new ActiveSession(deviceId, effectiveSessionId, profile, outputs.Create(profile));
            ActiveProfileChanged?.Invoke(profile.Name);
            return SwitchSessionResult.Success;
        }
        catch (DesktopInputException error)
        {
            return SwitchSessionResult.Failure(error.Code, error.Message);
        }
    }

    private async Task<SwitchSessionResult> ExecuteActiveLockedAsync(
        long sequence,
        Func<ISwitchOutputSession, Task> operation)
    {
        if (active!.Faulted)
        {
            return SwitchSessionResult.Failure("output_failure", "The switch output provider has faulted.");
        }
        if (sequence <= active.LastSequence)
        {
            return SwitchSessionResult.Success;
        }

        try
        {
            await operation(active.Output).ConfigureAwait(false);
            active.LastSequence = sequence;
            return SwitchSessionResult.Success;
        }
        catch
        {
            active.Faulted = true;
            return SwitchSessionResult.Failure("output_failure", "The switch output operation failed.");
        }
    }

    private async Task StopLockedAsync(CancellationToken token)
    {
        ActiveSession? previous = active;
        if (previous is not null)
        {
            await previous.Output.StopAsync(token).ConfigureAwait(false);
            active = null;
            ActiveProfileChanged?.Invoke(null);
        }
    }

    private sealed class ActiveSession(
        string deviceId,
        string sessionId,
        SwitchControlProfile profile,
        ISwitchOutputSession output)
    {
        public string DeviceId { get; } = deviceId;
        public string SessionId { get; } = sessionId;
        public SwitchControlProfile Profile { get; } = profile;
        public ISwitchOutputSession Output { get; } = output;
        public long LastSequence { get; set; }
        public bool Faulted { get; set; }
    }
}
