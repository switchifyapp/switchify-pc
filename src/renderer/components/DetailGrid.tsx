import type { ReactElement } from 'react';

export function TroubleshootingSection({
  title,
  children
}: {
  title: string;
  children: ReactElement | ReactElement[];
}): ReactElement {
  return (
    <section className="troubleshooting-section">
      <h3>{title}</h3>
      {children}
    </section>
  );
}

export function DetailGrid({ children }: { children: ReactElement | ReactElement[] }): ReactElement {
  return <div className="detail-grid">{children}</div>;
}

export function DetailItem({ label, value }: { label: string; value: string }): ReactElement {
  return (
    <div className="detail-item">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}
