namespace MinecraftServerDownloader.Ui.Bridge;

public sealed class OperationGate : IDisposable
{
    private readonly object _sync = new();
    private CancellationTokenSource? _current;

    public OperationLease Create()
    {
        lock (_sync)
        {
            if (_current is { IsCancellationRequested: false }) throw new InvalidOperationException("operation.running");
            _current?.Dispose();
            _current = new CancellationTokenSource();
            return new OperationLease(this, _current);
        }
    }

    public void CancelCurrent()
    {
        lock (_sync) _current?.Cancel();
    }

    public void Dispose()
    {
        lock (_sync)
        {
            _current?.Cancel();
            _current?.Dispose();
            _current = null;
        }
    }

    private void Complete(CancellationTokenSource source)
    {
        lock (_sync)
        {
            if (ReferenceEquals(_current, source)) _current = null;
        }
        source.Dispose();
    }

    public sealed class OperationLease : IDisposable
    {
        private readonly OperationGate _gate;
        private readonly CancellationTokenSource _source;
        private bool _disposed;

        internal OperationLease(OperationGate gate, CancellationTokenSource source)
        {
            _gate = gate;
            _source = source;
        }

        public CancellationToken Token => _source.Token;

        public void Dispose()
        {
            if (_disposed) return;
            _disposed = true;
            _gate.Complete(_source);
        }
    }
}


