"use strict";
var __webpack_require__ = {};
(()=>{
    __webpack_require__.d = (exports1, definition)=>{
        for(var key in definition)if (__webpack_require__.o(definition, key) && !__webpack_require__.o(exports1, key)) Object.defineProperty(exports1, key, {
            enumerable: true,
            get: definition[key]
        });
    };
})();
(()=>{
    __webpack_require__.o = (obj, prop)=>Object.prototype.hasOwnProperty.call(obj, prop);
})();
(()=>{
    __webpack_require__.r = (exports1)=>{
        if ('undefined' != typeof Symbol && Symbol.toStringTag) Object.defineProperty(exports1, Symbol.toStringTag, {
            value: 'Module'
        });
        Object.defineProperty(exports1, '__esModule', {
            value: true
        });
    };
})();
var __webpack_exports__ = {};
__webpack_require__.r(__webpack_exports__);
__webpack_require__.d(__webpack_exports__, {
    default: ()=>reporter
});
const external_node_fs_namespaceObject = require("node:fs");
const external_node_path_namespaceObject = require("node:path");
const agent_namespaceObject = require("@midscene/core/agent");
const report_namespaceObject = require("@midscene/core/report");
const common_namespaceObject = require("@midscene/shared/common");
const utils_namespaceObject = require("@midscene/shared/utils");
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
            const baseTag = `playwright-${(0, utils_namespaceObject.replaceIllegalPathCharsAndSpace)(testTitle)}`;
            const generatedFilename = (0, agent_namespaceObject.getReportFileName)(baseTag);
            this.testTitleToFilename.set(testTitle, generatedFilename);
        }
        return this.testTitleToFilename.get(testTitle);
    }
    getReportFilename(testTitle) {
        if ('merged' === this.mode) {
            if (!this.mergedFilename) this.mergedFilename = (0, agent_namespaceObject.getReportFileName)('playwright-merged');
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
        if ('html-and-external-assets' === this.outputFormat) return (0, external_node_path_namespaceObject.join)((0, common_namespaceObject.getMidsceneRunSubDir)('report'), fileName, 'index.html');
        return (0, external_node_path_namespaceObject.join)((0, common_namespaceObject.getMidsceneRunSubDir)('report'), `${fileName}.html`);
    }
    ensureOutputRoot() {
        (0, external_node_fs_namespaceObject.mkdirSync)((0, common_namespaceObject.getMidsceneRunSubDir)('report'), {
            recursive: true
        });
    }
    copyReport(reportFilePath, targetPath) {
        if ((0, report_namespaceObject.isDirectoryModeReport)(reportFilePath)) {
            const targetDir = (0, external_node_path_namespaceObject.dirname)(targetPath);
            (0, external_node_fs_namespaceObject.mkdirSync)(targetDir, {
                recursive: true
            });
            (0, external_node_fs_namespaceObject.cpSync)((0, external_node_path_namespaceObject.dirname)(reportFilePath), targetDir, {
                recursive: true,
                force: true
            });
            return;
        }
        (0, external_node_fs_namespaceObject.mkdirSync)((0, external_node_path_namespaceObject.dirname)(targetPath), {
            recursive: true
        });
        (0, external_node_fs_namespaceObject.copyFileSync)(reportFilePath, targetPath);
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
            if ((0, external_node_fs_namespaceObject.existsSync)(reportFilePath)) return true;
            (0, utils_namespaceObject.logMsg)(`Failed to read Midscene report file: ${reportFilePath}`, new Error('Report file does not exist'));
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
        const tool = new report_namespaceObject.ReportMergingTool();
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
                (0, agent_namespaceObject.printReportMsg)(targetPath);
                return;
            }
            const mergedReportPath = tool.mergeReports(targetName, {
                overwrite: true
            });
            if (mergedReportPath) (0, agent_namespaceObject.printReportMsg)(mergedReportPath);
            return;
        }
        const mergedReportPath = tool.mergeReports(targetName, {
            overwrite: true
        });
        if (mergedReportPath) (0, agent_namespaceObject.printReportMsg)(mergedReportPath);
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
                    (0, agent_namespaceObject.printReportMsg)(targetPath);
                    continue;
                }
                const tool = new report_namespaceObject.ReportMergingTool();
                tool.append(firstReport);
                const reportPath = tool.mergeReports(targetName, {
                    overwrite: true
                });
                if (reportPath) (0, agent_namespaceObject.printReportMsg)(reportPath);
                continue;
            }
            const tool = new report_namespaceObject.ReportMergingTool();
            for (const report of entry.reports)tool.append(report);
            const reportPath = tool.mergeReports(targetName, {
                overwrite: true
            });
            if (reportPath) (0, agent_namespaceObject.printReportMsg)(reportPath);
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
exports["default"] = __webpack_exports__["default"];
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "default"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});

//# sourceMappingURL=index.js.map