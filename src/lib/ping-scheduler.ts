/** Keep probes parallel without launching an unbounded number of temporary
 * Xray processes. HY2/QUIC handshakes are heavier than raw TCP probes and a
 * burst covering a large subscription can starve otherwise healthy probes. */
export const MAX_CONCURRENT_LOCATION_PINGS = 4;

export async function runPingsInParallel<T>(
  locations: readonly T[],
  ping: (location: T) => Promise<void>,
): Promise<void> {
  let next = 0;
  const worker = async () => {
    while (next < locations.length) {
      const location = locations[next++];
      await ping(location);
    }
  };
  await Promise.all(
    Array.from(
      { length: Math.min(MAX_CONCURRENT_LOCATION_PINGS, locations.length) },
      worker,
    ),
  );
}
