import type { SubGoal } from '../../types';
/**
 * Extract content from an XML tag in a string, searching from the end.
 * This approach handles cases where models prepend thinking content (like <think>...</think>)
 * before the actual response tags, or when there are incomplete/nested tags.
 *
 * Strategy: Find the LAST closing tag, then search backwards for the nearest opening tag.
 * This ensures we get the last complete tag pair, even if there are incomplete tags before it.
 *
 * @param xmlString - The XML string to parse
 * @param tagName - The name of the tag to extract (case-insensitive)
 * @returns The trimmed content of the tag, or undefined if not found
 */
export declare function extractXMLTag(xmlString: string, tagName: string): string | undefined;
/**
 * Parse sub-goals from XML content
 * Handles both formats:
 * - <sub-goal index="1" status="pending">description</sub-goal>
 * - <sub-goal index="1" status="finished" />
 */
export declare function parseSubGoalsFromXML(xmlContent: string): SubGoal[];
/**
 * Extract indexes of sub-goals marked as finished from <mark-sub-goal-done> content
 */
export declare function parseMarkFinishedIndexes(xmlContent: string): number[];
export declare const distanceThreshold = 16;
export declare function distance(point1: {
    x: number;
    y: number;
}, point2: {
    x: number;
    y: number;
}): number;
