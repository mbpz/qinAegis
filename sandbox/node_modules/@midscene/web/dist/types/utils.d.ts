/**
 * Get a require function that won't be processed by webpack.
 * Returns __non_webpack_require__ if available (in webpack environment),
 * otherwise falls back to the standard require.
 */
export declare function getWebpackRequire(): typeof require;
