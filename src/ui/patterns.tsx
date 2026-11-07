import { useId, type ComponentProps, type ReactNode } from "react";
import { ChevronRight } from "lucide-react";
import { Button } from "./controls";

export function FeedbackMessage({ children, tone = "error", variant = "notice", className = "", ...props }: ComponentProps<"div"> & { tone?: "error" | "status"; variant?: "inline" | "notice" | "banner" }) {
  return <div role={tone === "error" ? "alert" : "status"} {...props} className={`feedback-message feedback-${variant} ${className}`.trim()} data-tone={tone}>{children}</div>;
}

export function PageHeader({ title, description, action, before }: { title: string; description: ReactNode; action?: ReactNode; before?: ReactNode }) {
  return <header className="page-header"><div>{before}<h1>{title}</h1><p>{description}</p></div>{action}</header>;
}

export function SectionHeader({ title, description, action, headingId, ...props }: ComponentProps<"header"> & { title: string; description?: ReactNode; action?: ReactNode; headingId?: string }) {
  return <header {...props} className={`section-header ${props.className ?? ""}`.trim()}><div><h2 id={headingId}>{title}</h2>{description && <p>{description}</p>}</div>{action}</header>;
}

/** A whole-row action. Rows with several separate actions stay ordinary articles. */
export function ActionRow({ title, description, icon, trailing, className = "", ...props }: Omit<ComponentProps<"button">, "title"> & { title: string; description: ReactNode; icon?: ReactNode; trailing?: ReactNode }) {
  return <Button {...props} className={`action-row ${className}`.trim()}>{icon}<span className="action-row-text"><strong>{title}</strong><small>{description}</small></span>{trailing}<ChevronRight size={18} aria-hidden="true" /></Button>;
}

/** The control receives the IDs and validation state of its label and messages. */
export function FormField({ label, hint, error, children, className = "" }: { label: string; hint?: ReactNode; error?: string; className?: string; children: (props: { id: string; "aria-describedby"?: string; "aria-invalid"?: true }) => ReactNode }) {
  const id = useId();
  const describedBy = [hint && `${id}-hint`, error && `${id}-error`].filter(Boolean).join(" ") || undefined;
  return <div className={`field ${className}`.trim()}><label htmlFor={id}>{label}</label>{children({ id, "aria-describedby": describedBy, "aria-invalid": error ? true : undefined })}{hint && <p id={`${id}-hint`} className="setting-note">{hint}</p>}{error && <FeedbackMessage id={`${id}-error`} variant="inline">{error}</FeedbackMessage>}</div>;
}
