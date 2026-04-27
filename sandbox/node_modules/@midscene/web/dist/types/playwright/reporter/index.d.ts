import type { FullConfig, Reporter, Suite, TestCase, TestResult } from '@playwright/test/reporter';
interface MidsceneReporterOptions {
    type?: 'merged' | 'separate';
    outputFormat?: 'single-html' | 'html-and-external-assets';
}
declare class MidsceneReporter implements Reporter {
    private mergedFilename?;
    private testTitleToFilename;
    private reportsByTestId;
    mode?: 'merged' | 'separate';
    outputFormat: 'single-html' | 'html-and-external-assets';
    private hasMultipleProjects;
    constructor(options?: MidsceneReporterOptions);
    private static getMode;
    private getSeparatedFilename;
    private getReportFilename;
    private getReportPath;
    private ensureOutputRoot;
    private copyReport;
    private collectReportInfo;
    private finalizeMergedReport;
    private finalizeSeparateReports;
    onBegin(config: FullConfig, _suite: Suite): Promise<void>;
    onTestBegin(_test: TestCase, _result: TestResult): void;
    onTestEnd(test: TestCase, result: TestResult): void;
    onEnd(): Promise<void>;
}
export default MidsceneReporter;
