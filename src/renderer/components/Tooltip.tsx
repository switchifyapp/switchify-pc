import { cloneElement, useId, type ReactElement } from 'react';

type TooltipChildProps = {
  'aria-describedby'?: string;
};

export function Tooltip({
  label,
  placement = 'top',
  children
}: {
  label: string;
  placement?: 'top' | 'bottom';
  children: ReactElement;
}): ReactElement {
  const tooltipId = useId();
  const describedChild = children as ReactElement<TooltipChildProps>;

  return (
    <span className={`tooltip-anchor tooltip-anchor-${placement}`} data-tooltip={label}>
      {cloneElement(describedChild, {
        'aria-describedby': tooltipId
      })}
      <span id={tooltipId} role="tooltip" className="tooltip-bubble">
        {label}
      </span>
    </span>
  );
}
