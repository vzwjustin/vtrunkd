## 2026-05-05 - Parallelized Health Pings and Broadcast

**Learning:** Sequential awaits in a loop for network I/O operations create a bottleneck that scales linearly with the number of links and their latency. Using `tokio::task::JoinSet` allows these operations to run concurrently, reducing the total time from $O(\sum L_i)$ to $O(\max L_i)$.

**Action:** Always check if independent asynchronous operations in a loop can be parallelized, especially for networking and health checks. Use `Arc` to share sockets and data across spawned tasks safely.
