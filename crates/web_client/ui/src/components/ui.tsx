// Application adapters over unmodified, registry-installed beUI components.
import {
  Children, Fragment, isValidElement, useId, useLayoutEffect, useRef, useState,
  type ReactNode, type OptionHTMLAttributes, type CSSProperties,
} from 'react';
import { Button as BeButton, type ButtonProps } from './motion/button/base';
import { Input as BeInput, type InputProps as BeInputProps } from './motion/input';
import { Checkbox as BeCheckbox, type CheckboxProps } from './motion/checkbox';
import { Loader as BeLoader, type LoaderProps } from './motion/loader';
import {
  Select as BeSelect, SelectTrigger, SelectContent, SelectItem, SelectValue,
} from './motion/select';
import { PromptInput, type PromptInputProps } from './agents/prompt-input';
import { cn } from '@/lib/utils';

export type { ButtonProps, CheckboxProps, LoaderProps };
export function Button({ className, variant, ...props }: ButtonProps) {
  const classes = new Set(className?.split(/\s+/));
  const mapped = classes.has('btn-primary') ? 'primary'
    : classes.has('btn-secondary') ? 'secondary'
    : classes.has('btn-danger') ? 'outline'
    : classes.has('btn') || classes.has('sidebar-item') ? 'ghost' : 'primary';
  return <BeButton type="button" {...props} variant={variant ?? mapped}
    className={cn(classes.has('btn-danger') && 'border-destructive/40 text-destructive hover:bg-destructive/10', className)} />;
}
export function Checkbox(props: CheckboxProps) { return <BeCheckbox {...props} />; }
export function Loader(props: LoaderProps) { return <BeLoader size={20} label="正在加载" {...props} />; }

export interface InputProps extends Omit<BeInputProps, 'value' | 'defaultValue' | 'onChange'> {
  value?: string | number;
  defaultValue?: string | number;
  onValueChange?: (value: string) => void;
}
function fieldClass(className?: string) { return className?.split(/\s+/).filter((token) => token !== 'input').join(' '); }
export function Input({ value, defaultValue, onValueChange, className, classNames, ...props }: InputProps) {
  return <BeInput {...props} value={value === undefined ? undefined : String(value)}
    defaultValue={defaultValue === undefined ? undefined : String(defaultValue)}
    onChange={onValueChange} className={cn('beui-input', fieldClass(className))}
    classNames={{ ...classNames, field: cn('rounded-xl', classNames?.field) }} />;
}

export interface TextAreaProps extends Omit<PromptInputProps, 'onSubmit' | 'onValueChange' | 'value'> {
  value: string;
  onValueChange: (value: string) => void;
}
export function TextArea({ value, onValueChange, rows = 4, minRows, maxRows, className, onKeyDown, readOnly, id, 'aria-label': ariaLabel, ...props }: TextAreaProps) {
  const [externalLabel, setExternalLabel] = useState<string>();
  useLayoutEffect(() => {
    const labels = id ? Array.from(document.querySelectorAll<HTMLLabelElement>(`label[for="${CSS.escape(id)}"]`)) : [];
    setExternalLabel(labels.map((label) => label.textContent?.trim()).filter(Boolean).join(' ') || undefined);
  }, [id]);
  return <PromptInput {...props} value={value} onValueChange={onValueChange} readOnly={readOnly}
    placeholder={props.placeholder ?? ''}
    id={id} aria-label={ariaLabel ?? externalLabel ?? props.placeholder ?? '多行内容'}
    minRows={minRows ?? rows} maxRows={maxRows ?? Math.max(rows, 18)}
    className={cn('beui-textarea [&>div:last-child]:hidden', fieldClass(className))}
    onKeyDown={(event) => {
      onKeyDown?.(event);
      if (event.defaultPrevented || event.key !== 'Enter' || event.shiftKey || event.nativeEvent.isComposing) return;
      // The official composer submits Enter. Multiline product fields instead
      // insert a newline, preserving the real textarea selection and callback.
      event.preventDefault();
      if (readOnly) return;
      const textarea = event.currentTarget;
      const start = textarea.selectionStart;
      const end = textarea.selectionEnd;
      onValueChange(`${value.slice(0, start)}\n${value.slice(end)}`);
      requestAnimationFrame(() => textarea.setSelectionRange(start + 1, start + 1));
    }} />;
}

export interface SelectProps {
  value: string;
  onValueChange: (value: string) => void;
  id?: string;
  name?: string;
  'aria-label'?: string;
  'aria-describedby'?: string;
  disabled?: boolean;
  required?: boolean;
  className?: string;
  style?: CSSProperties;
  children: ReactNode;
}

function textContent(children: ReactNode): string {
  return Children.toArray(children).map((child) => {
    if (typeof child === 'string' || typeof child === 'number') return String(child);
    return isValidElement<{ children?: ReactNode }>(child) ? textContent(child.props.children) : '';
  }).join('');
}

function options(children: ReactNode, groupDisabled = false): { value: string; label: string; disabled: boolean }[] {
  return Children.toArray(children).flatMap((child) => {
    if (!isValidElement<OptionHTMLAttributes<HTMLOptionElement> & { children?: ReactNode }>(child)) return [];
    if (child.type === Fragment || child.type === 'optgroup') {
      return options(child.props.children, groupDisabled || Boolean(child.props.disabled));
    }
    if (child.type !== 'option') return [];
    const label = textContent(child.props.children);
    return [{ value: String(child.props.value ?? label), label,
      disabled: groupDisabled || Boolean(child.props.disabled) }];
  });
}

export function Select({ value, onValueChange, id, name, disabled, required, className, style, children,
  'aria-label': ariaLabel, 'aria-describedby': describedBy }: SelectProps) {
  const root = useRef<HTMLDivElement>(null);
  const autoId = useId();
  const valueLabelId = `${autoId}-value-label`;
  const items = options(children);
  useLayoutEffect(() => {
    const trigger = root.current?.querySelector<HTMLButtonElement>('button[aria-haspopup="listbox"]');
    if (!trigger) return;
    if (ariaLabel) trigger.setAttribute('aria-label', ariaLabel);
    else trigger.setAttribute('aria-labelledby', valueLabelId);
    if (describedBy) trigger.setAttribute('aria-describedby', describedBy);
    if (required) trigger.setAttribute('aria-required', 'true');
    // Keep beUI's generated trigger ID: its own focus recovery relies on it.
    // Retarget real external labels rather than assigning an ID to a wrapper.
    const labels = id ? Array.from(document.querySelectorAll<HTMLLabelElement>(`label[for="${CSS.escape(id)}"]`)) : [];
    const linked = labels.map((label, index) => {
      const oldId = label.id;
      if (!label.id) label.id = `${autoId}-external-label-${index}`;
      label.htmlFor = trigger.id;
      return { label, oldId };
    });
    if (!ariaLabel && linked.length) trigger.setAttribute('aria-labelledby', linked.map(({ label }) => label.id).join(' '));
    return () => {
      for (const { label, oldId } of linked) { label.htmlFor = id!; label.id = oldId; }
      trigger.removeAttribute('aria-label');
      trigger.removeAttribute('aria-labelledby');
      trigger.removeAttribute('aria-describedby');
      trigger.removeAttribute('aria-required');
    };
  }, [id, autoId, ariaLabel, describedBy, required, valueLabelId]);
  return <div ref={root} className={cn('beui-select', fieldClass(className))} style={style}>
    <BeSelect value={value} onValueChange={onValueChange} disabled={disabled}>
      <SelectTrigger><span id={valueLabelId}><SelectValue placeholder="请选择" /></span></SelectTrigger>
      <SelectContent className="max-h-72 overflow-y-auto">
        {items.map((item) => <SelectItem key={item.value} value={item.value} disabled={item.disabled}>{item.label}</SelectItem>)}
      </SelectContent>
    </BeSelect>
    {(name || required) && <input className="sr-only" aria-hidden="true" tabIndex={-1}
      name={name} value={value} required={required} disabled={disabled} onChange={() => {}}
      onInvalid={(event) => {
        event.preventDefault();
        root.current?.querySelector<HTMLButtonElement>('button[aria-haspopup="listbox"]')?.focus();
      }} />}
  </div>;
}
