type TSharpModule = typeof import('sharp');
export default function getSharp(): Promise<TSharpModule>;
export {};
