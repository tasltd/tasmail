export function LoadingSkeleton({ rows = 5 }: { rows?: number }) {
  return (
    <div className="loading-skeleton">
      {Array.from({ length: rows }).map((_, i) => (
        <div key={i} className="skeleton-row" />
      ))}
    </div>
  );
}
