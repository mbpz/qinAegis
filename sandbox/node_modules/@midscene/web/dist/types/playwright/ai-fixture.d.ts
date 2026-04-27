import { type PlaywrightWebPage } from './index';
import type { WebPageAgentOpt } from '../web-element';
import type { Agent as PageAgent } from '@midscene/core/agent';
import { type TestInfo, type TestType } from '@playwright/test';
import type { Page as OriginPlaywrightPage } from 'playwright';
export type APITestType = Pick<TestType<any, any>, 'step'>;
export declare const midsceneDumpAnnotationId = "MIDSCENE_DUMP_ANNOTATION";
type PlaywrightCacheConfig = {
    strategy?: 'read-only' | 'read-write' | 'write-only';
    id?: string;
};
type PlaywrightCache = false | true | PlaywrightCacheConfig;
export type PlaywrightAiFixtureOptions = Omit<WebPageAgentOpt, 'testId' | 'cacheId' | 'groupName' | 'groupDescription' | 'reportFileName' | 'cache'> & {
    cache?: PlaywrightCache;
};
export declare const PlaywrightAiFixture: (options?: PlaywrightAiFixtureOptions) => {
    _midsceneFinalizeReports: ((({}: Record<string, unknown>, use: any, testInfo: TestInfo) => Promise<void>) | {
        auto: boolean;
    })[];
    agentForPage: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    ai: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    aiAct: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    /**
     * @deprecated Use {@link PlaywrightAiFixture.aiAct} instead.
     */
    aiAction: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    aiTap: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    aiRightClick: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    aiDoubleClick: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    aiHover: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    aiInput: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    aiKeyboardPress: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    aiScroll: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    aiQuery: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    aiAssert: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    aiWaitFor: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    aiLocate: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    aiNumber: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    aiString: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    aiBoolean: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    aiAsk: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    runYaml: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    setAIActionContext: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    evaluateJavaScript: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    recordToReport: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    logScreenshot: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    freezePageContext: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
    unfreezePageContext: ({ page }: {
        page: OriginPlaywrightPage;
    }, use: any, testInfo: TestInfo) => Promise<void>;
};
export type PlayWrightAiFixtureType = {
    agentForPage: (page?: OriginPlaywrightPage, opts?: WebPageAgentOpt) => Promise<PageAgent<PlaywrightWebPage>>;
    ai: <T = any>(...args: Parameters<PageAgent['ai']>) => Promise<T>;
    aiAct: (...args: Parameters<PageAgent['aiAct']>) => ReturnType<PageAgent['aiAct']>;
    /**
     * @deprecated Use {@link PlayWrightAiFixtureType.aiAct} instead.
     */
    aiAction: (...args: Parameters<PageAgent['aiAction']>) => ReturnType<PageAgent['aiAction']>;
    aiTap: (...args: Parameters<PageAgent['aiTap']>) => ReturnType<PageAgent['aiTap']>;
    aiRightClick: (...args: Parameters<PageAgent['aiRightClick']>) => ReturnType<PageAgent['aiRightClick']>;
    aiDoubleClick: (...args: Parameters<PageAgent['aiDoubleClick']>) => ReturnType<PageAgent['aiDoubleClick']>;
    aiHover: (...args: Parameters<PageAgent['aiHover']>) => ReturnType<PageAgent['aiHover']>;
    aiInput: (...args: Parameters<PageAgent['aiInput']>) => ReturnType<PageAgent['aiInput']>;
    aiKeyboardPress: (...args: Parameters<PageAgent['aiKeyboardPress']>) => ReturnType<PageAgent['aiKeyboardPress']>;
    aiScroll: (...args: Parameters<PageAgent['aiScroll']>) => ReturnType<PageAgent['aiScroll']>;
    aiQuery: <T = any>(...args: Parameters<PageAgent['aiQuery']>) => Promise<T>;
    aiAssert: (...args: Parameters<PageAgent['aiAssert']>) => ReturnType<PageAgent['aiAssert']>;
    aiWaitFor: (...args: Parameters<PageAgent['aiWaitFor']>) => Promise<void>;
    aiLocate: (...args: Parameters<PageAgent['aiLocate']>) => ReturnType<PageAgent['aiLocate']>;
    aiNumber: (...args: Parameters<PageAgent['aiNumber']>) => ReturnType<PageAgent['aiNumber']>;
    aiString: (...args: Parameters<PageAgent['aiString']>) => ReturnType<PageAgent['aiString']>;
    aiBoolean: (...args: Parameters<PageAgent['aiBoolean']>) => ReturnType<PageAgent['aiBoolean']>;
    aiAsk: (...args: Parameters<PageAgent['aiAsk']>) => ReturnType<PageAgent['aiAsk']>;
    runYaml: (...args: Parameters<PageAgent['runYaml']>) => ReturnType<PageAgent['runYaml']>;
    setAIActionContext: (...args: Parameters<PageAgent['setAIActionContext']>) => ReturnType<PageAgent['setAIActionContext']>;
    evaluateJavaScript: (...args: Parameters<PageAgent['evaluateJavaScript']>) => ReturnType<PageAgent['evaluateJavaScript']>;
    recordToReport: (...args: Parameters<PageAgent['recordToReport']>) => ReturnType<PageAgent['recordToReport']>;
    logScreenshot: (...args: Parameters<PageAgent['logScreenshot']>) => ReturnType<PageAgent['logScreenshot']>;
    freezePageContext: (...args: Parameters<PageAgent['freezePageContext']>) => ReturnType<PageAgent['freezePageContext']>;
    unfreezePageContext: (...args: Parameters<PageAgent['unfreezePageContext']>) => ReturnType<PageAgent['unfreezePageContext']>;
};
export {};
