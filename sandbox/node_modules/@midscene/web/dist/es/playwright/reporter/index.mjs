import { copyFileSync, cpSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { getReportFileName, printReportMsg } from "@midscene/core/agent";
import { ReportMergingTool, isDirectoryModeReport } from "@midscene/core/report";
import { getMidsceneRunSubDir } from "@midscene/shared/common";
import { logMsg, replaceIllegalPathCharsAndSpace } from "@midscene/shared/utils";
function _define_property(obj, key, value) {
    if (key in obj) Object.defineProperty(obj, key, {
        value: value,
        enumerable: true,
        configurable: true,
        writable: true
    });
    else obj[key] = value;
    return obj;
}
class MidsceneReporter {
    static getMode(reporterType) {
        if (!reporterType) return 'merged';
        if ('merged' !== reporterType && 'separate' !== reporterType) throw new Error(`Unknown reporter type in playwright config: ${reporterType}, only support 'merged' or 'separate'`);
        return reporterType;
    }
    getSeparatedFilename(testTitle) {
        if (!this.testTitleToFilename.has(testTitle)) {
            const baseTag = `playwright-${replaceIllegalPathCharsAndSpace(testTitle)}`;
            const generatedFilename = getReportFileName(baseTag);
            this.testTitleToFilename.set(testTitle, generatedFilename);
        }
        return this.testTitleToFilename.get(testTitle);
    }
    getReportFilename(testTitle) {
        if ('merged' === this.mode) {
            if (!this.mergedFilename) this.mergedFilename = getReportFileName('playwright-merged');
            return this.mergedFilename;
        }
        if ('separate' === this.mode) {
            if (!testTitle) throw new Error('testTitle is required in separate mode');
            return this.getSeparatedFilename(testTitle);
        }
        throw new Error(`Unknown mode: ${this.mode}`);
    }
    getReportPath(testTitle) {
        const fileName = this.getReportFilename(testTitle);
        if ('html-and-external-assets' === this.outputFormat) return join(getMidsceneRunSubDir('report'), fileName, 'index.html');
        return join(getMidsceneRunSubDir('report'), `${fileName}.html`);
    }
    ensureOutputRoot() {
        mkdirSync(getMidsceneRunSubDir('report'), {
            recursive: true
        });
    }
    copyReport(reportFilePath, targetPath) {
        if (isDirectoryModeReport(reportFilePath)) {
            const targetDir = dirname(targetPath);
            mkdirSync(targetDir, {
                recursive: true
            });
            cpSync(dirname(reportFilePath), targetDir, {
                recursive: true,
                force: true
            });
            return;
        }
        mkdirSync(dirname(targetPath), {
            recursive: true
        });
        copyFileSync(reportFilePath, targetPath);
    }
    collectReportInfo(test, result) {
        const reportAnnotations = test.annotations.filter((annotation)=>'MIDSCENE_DUMP_ANNOTATION' === annotation.type && annotation.description);
        if (0 === reportAnnotations.length || !this.mode) return;
        const retry = result.retry ? `(retry #${result.retry})` : '';
        const testId = `${test.id}${retry}`;
        const projectName = this.hasMultipleProjects ? test.parent?.project()?.name : void 0;
        const projectSuffix = projectName ? ` [${projectName}]` : '';
        const testTitle = `${test.title}${projectSuffix}${retry}`;
        const reports = reportAnnotations.map((annotation)=>annotation.description).filter((reportFilePath)=>{
            if (existsSync(reportFilePath)) return true;
            logMsg(`Failed to read Midscene report file: ${reportFilePath}`, new Error('Report file does not exist'));
            return false;
        }).map((reportFilePath)=>({
                reportFilePath,
                reportAttributes: {
                    testDuration: result.duration,
                    testStatus: result.status,
                    testTitle,
                    testId,
                    testDescription: test.parent?.title || ''
                }
            }));
        if (0 === reports.length) return;
        this.reportsByTestId.set(testId, {
            testTitle,
            reports
        });
    }
    finalizeMergedReport() {
        this.ensureOutputRoot();
        const tool = new ReportMergingTool();
        let reportCount = 0;
        for (const entry of this.reportsByTestId.values())for (const report of entry.reports){
            tool.append(report);
            reportCount += 1;
        }
        if (0 === reportCount) return;
        const targetName = this.getReportFilename();
        if (1 === reportCount) {
            const firstReport = Array.from(this.reportsByTestId.values())[0]?.reports[0];
            if (!firstReport) return;
            if (firstReport.reportFilePath) {
                const targetPath = this.getReportPath();
                this.copyReport(firstReport.reportFilePath, targetPath);
                printReportMsg(targetPath);
                return;
            }
            const mergedReportPath = tool.mergeReports(targetName, {
                overwrite: true
            });
            if (mergedReportPath) printReportMsg(mergedReportPath);
            return;
        }
        const mergedReportPath = tool.mergeReports(targetName, {
            overwrite: true
        });
        if (mergedReportPath) printReportMsg(mergedReportPath);
    }
    finalizeSeparateReports() {
        this.ensureOutputRoot();
        for (const entry of this.reportsByTestId.values()){
            const targetName = this.getReportFilename(entry.testTitle);
            if (1 === entry.reports.length) {
                const firstReport = entry.reports[0];
                if (firstReport.reportFilePath) {
                    const targetPath = this.getReportPath(entry.testTitle);
                    this.copyReport(firstReport.reportFilePath, targetPath);
                    printReportMsg(targetPath);
                    continue;
                }
                const tool = new ReportMergingTool();
                tool.append(firstReport);
                const reportPath = tool.mergeReports(targetName, {
                    overwrite: true
                });
                if (reportPath) printReportMsg(reportPath);
                continue;
            }
            const tool = new ReportMergingTool();
            for (const report of entry.reports)tool.append(report);
            const reportPath = tool.mergeReports(targetName, {
                overwrite: true
            });
            if (reportPath) printReportMsg(reportPath);
        }
    }
    async onBegin(config, _suite) {
        this.hasMultipleProjects = (config.projects?.length || 0) > 1;
    }
    onTestBegin(_test, _result) {}
    onTestEnd(test, result) {
        this.collectReportInfo(test, result);
    }
    async onEnd() {
        if ('merged' === this.mode) return void this.finalizeMergedReport();
        if ('separate' === this.mode) this.finalizeSeparateReports();
    }
    constructor(options = {}){
        _define_property(this, "mergedFilename", void 0);
        _define_property(this, "testTitleToFilename", new Map());
        _define_property(this, "reportsByTestId", new Map());
        _define_property(this, "mode", void 0);
        _define_property(this, "outputFormat", void 0);
        _define_property(this, "hasMultipleProjects", false);
        this.mode = MidsceneReporter.getMode(options.type ?? 'merged');
        this.outputFormat = options.outputFormat ?? 'single-html';
    }
}
const reporter = MidsceneReporter;
export { reporter as default };

//# sourceMappingURL=index.mjs.map