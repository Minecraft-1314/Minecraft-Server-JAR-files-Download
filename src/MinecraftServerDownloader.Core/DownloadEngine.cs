using System.Security.Cryptography;

namespace MinecraftServerDownloader.Core;

public sealed class DownloadEngine
{
    private const int BufferSize = 131_072;
    private const long EstimatePerFile = 41_943_040;
    private readonly SafeHttp _http;
    private readonly VersionCatalog _catalog;
    private readonly PauseController _pause = new();
    private CancellationTokenSource? _source;

    public DownloadEngine(SafeHttp http, VersionCatalog catalog)
    {
        _http = http;
        _catalog = catalog;
    }

    public void Pause() => _pause.Pause();

    public void Resume() => _pause.Resume();

    public void Stop() => _source?.Cancel();

    public async Task<DownloadSummary> RunAsync(IReadOnlyList<VersionInfo> versions, AppSettings settings, IReadOnlyList<SourceProfile> sources, IProgress<DownloadProgress> progress, Action<string, bool> log, CancellationToken cancellationToken)
    {
        using CancellationTokenSource linked = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        _source = linked;
        _pause.Resume();
        long totalBytes = 0;
        long doneBytes = 0;
        int completed = 0;
        int failed = 0;
        object sync = new();
        try
        {
            List<VersionJob> jobs = await ResolveJobsAsync(versions, settings, sources, log, linked.Token).ConfigureAwait(false);
            foreach (VersionJob job in jobs.Where(item => !item.Version.IsDownloaded)) totalBytes += job.Size > 0 ? job.Size : EstimatePerFile;
            if (jobs.Count == 0 || jobs.All(item => item.Version.IsDownloaded))
            {
                log("download.allDone", false);
                return new DownloadSummary(DownloadOutcome.Completed, versions.Count, 0, 0);
            }
            if (DiskSpace.AvailableBytes(settings.DownloadDirectory) < totalBytes)
            {
                log("download.diskFull", true);
                return new DownloadSummary(DownloadOutcome.DiskFull, completed, 0, totalBytes);
            }
            await Parallel.ForEachAsync(jobs, new ParallelOptions { MaxDegreeOfParallelism = settings.MaxConcurrent, CancellationToken = linked.Token }, async (job, token) =>
            {
                if (job.Version.IsDownloaded)
                {
                    Interlocked.Increment(ref completed);
                    return;
                }
                try
                {
                    long written = await DownloadOneAsync(job, settings.DownloadDirectory, bytes => Interlocked.Add(ref doneBytes, bytes), log, token).ConfigureAwait(false);
                    Interlocked.Increment(ref completed);
                    Interlocked.Add(ref totalBytes, Math.Max(0, written - (job.Size > 0 ? job.Size : EstimatePerFile)));
                    lock (sync) progress.Report(new DownloadProgress(Interlocked.CompareExchange(ref completed, 0, 0), versions.Count, Interlocked.CompareExchange(ref doneBytes, 0, 0), Interlocked.CompareExchange(ref totalBytes, 0, 0), 0, 0));
                }
                catch (OperationCanceledException) when (linked.IsCancellationRequested) { }
                catch
                {
                    Interlocked.Increment(ref failed);
                    log("download.failed:" + job.Version.JarName, true);
                }
            }).ConfigureAwait(false);
            DownloadOutcome outcome = linked.IsCancellationRequested ? DownloadOutcome.Stopped : failed > 0 ? DownloadOutcome.Failed : DownloadOutcome.Completed;
            progress.Report(new DownloadProgress(completed, versions.Count, Interlocked.Read(ref doneBytes), Interlocked.Read(ref totalBytes), 0, 0));
            log(outcome switch { DownloadOutcome.Stopped => "download.stopped", DownloadOutcome.Failed => "download.finishedWithErrors", _ => "download.allDone" }, outcome == DownloadOutcome.Failed);
            return new DownloadSummary(outcome, completed, failed, Interlocked.Read(ref totalBytes));
        }
        catch (OperationCanceledException)
        {
            progress.Report(new DownloadProgress(completed, versions.Count, Interlocked.Read(ref doneBytes), Interlocked.Read(ref totalBytes), 0, 0));
            log("download.stopped", false);
            return new DownloadSummary(DownloadOutcome.Stopped, completed, failed, Interlocked.Read(ref totalBytes));
        }
        finally
        {
            _source = null;
        }
    }

    private async Task<List<VersionJob>> ResolveJobsAsync(IReadOnlyList<VersionInfo> versions, AppSettings settings, IReadOnlyList<SourceProfile> sources, Action<string, bool> log, CancellationToken cancellationToken)
    {
        List<VersionJob> jobs = []; object jobsSync = new();
        await Parallel.ForEachAsync(versions, new ParallelOptions { MaxDegreeOfParallelism = settings.MaxConcurrent, CancellationToken = cancellationToken }, async (version, token) =>
        {
            try
            {
                VersionJob? job = await _catalog.ResolveAsync(settings, sources, version, token).ConfigureAwait(false);
                if (job is null) log("download.noUrl:" + version.Id, true);
                else lock (jobsSync) jobs.Add(job);
            }
            catch (OperationCanceledException) { throw; }
            catch { log("download.metadataFailed:" + version.Id, true); }
        }).ConfigureAwait(false);
        return jobs;
    }

    private async Task<long> DownloadOneAsync(VersionJob job, string root, Action<long> reportBytes, Action<string, bool> log, CancellationToken cancellationToken)
    {
        string directory = Path.Combine(InputValidator.NormalizeDirectory(root), job.Version.Folder);
        Directory.CreateDirectory(directory);
        string finalPath = Path.Combine(directory, job.Version.JarName);
        string partPath = finalPath + ".part";
        log("download.starting:" + job.Version.JarName, false);
        int attempts = 0;
        while (true)
        {
            cancellationToken.ThrowIfCancellationRequested();
            await _pause.WaitAsync(cancellationToken).ConfigureAwait(false);
            try
            {
                long offset = File.Exists(partPath) ? new FileInfo(partPath).Length : 0;
                if (job.Size > 0 && offset > job.Size)
                {
                    File.Delete(partPath);
                    offset = 0;
                }
                using HttpResponseMessage response = await _http.GetDownloadAsync(new Uri(job.ServerUrl), offset, cancellationToken).ConfigureAwait(false);
                if (offset > 0 && response.StatusCode != System.Net.HttpStatusCode.PartialContent)
                {
                    offset = 0;
                }
                long expected = ExpectedTotal(response, job.Size, offset);
                await using Stream input = await response.Content.ReadAsStreamAsync(cancellationToken).ConfigureAwait(false);
                await using FileStream output = new(partPath, offset > 0 ? FileMode.Append : FileMode.Create, FileAccess.Write, FileShare.None, BufferSize, FileOptions.Asynchronous | FileOptions.SequentialScan);
                byte[] buffer = new byte[BufferSize];
                long written = offset;
                while (true)
                {
                    cancellationToken.ThrowIfCancellationRequested();
                    await _pause.WaitAsync(cancellationToken).ConfigureAwait(false);
                    int read = await input.ReadAsync(buffer, cancellationToken).ConfigureAwait(false);
                    if (read == 0) break;
                    await output.WriteAsync(buffer.AsMemory(0, read), cancellationToken).ConfigureAwait(false);
                    written += read;
                    reportBytes(read);
                }
                await output.FlushAsync(cancellationToken).ConfigureAwait(false);
                if (expected > 0 && written != expected) throw new IOException("incomplete");
                if (job.Sha1.Length > 0 && !await VerifySha1Async(partPath, job.Sha1, cancellationToken).ConfigureAwait(false))
                {
                    File.Delete(partPath);
                    throw new InvalidDataException("checksum");
                }
                File.Move(partPath, finalPath, true);
                log("download.completed:" + job.Version.JarName, false);
                return written;
            }
            catch (OperationCanceledException) { throw; }
            catch (Exception error) when (error is IOException or HttpRequestException or InvalidDataException && attempts < 3 && !cancellationToken.IsCancellationRequested)
            {
                attempts++;
                await Task.Delay(attempts * 750, cancellationToken).ConfigureAwait(false);
            }
        }
    }

    private static long ExpectedTotal(HttpResponseMessage response, long expected, long offset)
    {
        if (response.Headers.TryGetValues("Content-Range", out IEnumerable<string>? values))
        {
            string? value = values.FirstOrDefault();
            int slash = value?.LastIndexOf('/') ?? -1;
            if (slash >= 0 && long.TryParse(value![(slash + 1)..], out long total)) return total;
        }
        if (expected > 0) return expected;
        return response.Content.Headers.ContentLength is > 0 ? offset + response.Content.Headers.ContentLength.Value : 0;
    }

    [System.Diagnostics.CodeAnalysis.SuppressMessage("Security", "CA5350", Justification = "Minecraft manifests publish SHA-1 checksums.")]
    private static async Task<bool> VerifySha1Async(string path, string expected, CancellationToken cancellationToken)
    {
        await using FileStream stream = new(path, FileMode.Open, FileAccess.Read, FileShare.Read, BufferSize, FileOptions.Asynchronous | FileOptions.SequentialScan);
        byte[] hash = await SHA1.HashDataAsync(stream, cancellationToken).ConfigureAwait(false);
        return Convert.ToHexString(hash).Equals(expected, StringComparison.OrdinalIgnoreCase);
    }

    private sealed class PauseController
    {
        private readonly object _sync = new();
        private TaskCompletionSource _resumed = CreateCompleted();

        public void Pause()
        {
            lock (_sync)
            {
                if (!_resumed.Task.IsCompleted) return;
                _resumed = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
            }
        }

        public void Resume()
        {
            lock (_sync) _resumed.TrySetResult();
        }

        public Task WaitAsync(CancellationToken cancellationToken)
        {
            Task task;
            lock (_sync) task = _resumed.Task;
            return task.WaitAsync(cancellationToken);
        }

        private static TaskCompletionSource CreateCompleted()
        {
            TaskCompletionSource source = new(TaskCreationOptions.RunContinuationsAsynchronously);
            source.SetResult();
            return source;
        }
    }
}





