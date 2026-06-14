using SwitchifyBluetoothTransport;

using var shutdown = new CancellationTokenSource();
var service = new BluetoothGattBridge();

Console.CancelKeyPress += (_, eventArgs) =>
{
    eventArgs.Cancel = true;
    shutdown.Cancel();
};

try
{
    while (!shutdown.IsCancellationRequested)
    {
        var line = await Console.In.ReadLineAsync(shutdown.Token);
        if (line is null)
        {
            break;
        }

        if (string.IsNullOrWhiteSpace(line))
        {
            continue;
        }

        HelperCommand? command;
        try
        {
            command = HelperProtocol.ParseCommand(line);
        }
        catch
        {
            HelperProtocol.WriteEvent(new { type = "error", reason = "invalid_command" });
            continue;
        }

        switch (command)
        {
            case StartCommand start:
                await service.StartAsync(start);
                break;
            case StopCommand:
                service.Stop();
                break;
            case SendCommand send:
                await service.SendAsync(send);
                break;
            case DisconnectCommand disconnect:
                service.Disconnect(disconnect.ConnectionId);
                break;
            case ShutdownCommand:
                shutdown.Cancel();
                break;
            default:
                HelperProtocol.WriteEvent(new { type = "error", reason = "unsupported_command" });
                break;
        }
    }
}
catch (OperationCanceledException)
{
    // Normal shutdown.
}
finally
{
    service.Stop();
}

