declare module 'js-yaml' {
  export interface LoadOptions {
    filename?: string;
    onWarning?: (warning: Error) => void;
    schema?: unknown;
  }
  export function load(str: string, options?: LoadOptions): unknown;
  export function dump(obj: unknown): string;
}