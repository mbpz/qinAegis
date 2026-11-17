import { createRequire } from 'node:module';
import { Worker } from 'node:worker_threads';
import { lstat, mkdir, readFile, readdir, realpath, chmod, mkdtemp, writeFile, rename, rm } from 'node:fs/promises';
import { isAbsolute, dirname, basename, join, parse, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
// Awesome's pinned template always enables optional analytics. Keep its report payload
// unchanged while removing those exact tracking and document-base bootstraps.
const OFFLINE_CSP = "default-src 'none'; script-src 'unsafe-inline' blob: data:; style-src 'unsafe-inline' blob: data:; img-src data: blob:; font-src data: blob:; connect-src blob: data:; worker-src blob:; base-uri 'none'; form-action 'none'";
function offlineHtml(html:string):string {
  html=html.replace(/<script async src="https:\/\/www\.googletagmanager\.com\/gtag\/js\?id=G-LNDJ3J7WT0"><\/script>/g,'')
    .replace(/<script>\s*window\.dataLayer = window\.dataLayer \|\| \[\];\s*function gtag\(\)\{dataLayer\.push\(arguments\);\}\s*gtag\('js', new Date\(\)\);\s*gtag\('config', 'G-LNDJ3J7WT0', \{[\s\S]*?\}\);\s*<\/script>/g,'')
    .replace(/<script>\s*const url = new URL\(window\.location\.href\);\s*url\.search = "";\s*url\.hash = "";\s*const baseEl = document\.createElement\("base"\);\s*baseEl\.href = url\.toString\(\);\s*window\.document\.head\.appendChild\(baseEl\);\s*<\/script>/g,'')
    .replace(/<base\b[^>]*>/gi,'');
  if(html.includes('www.googletagmanager.com')||html.includes('function gtag()')||html.includes('const baseEl ='))throw new Error('Unrecognized Allure template');
  const storage=`<script>for(const key of ['localStorage','sessionStorage']){try{void window[key]}catch{const entries=new Map();Object.defineProperty(window,key,{configurable:true,value:{getItem:k=>entries.get(String(k))??null,setItem:(k,v)=>entries.set(String(k),String(v)),removeItem:k=>entries.delete(String(k)),clear:()=>entries.clear(),key:i=>Array.from(entries.keys())[i]??null,get length(){return entries.size}}})}}</script>`;
  if(!html.includes('<head>'))throw new Error('Invalid Allure HTML');
  return html.replace('<head>',`<head><meta http-equiv="Content-Security-Policy" content="${OFFLINE_CSP}">${storage}`);
}
async function safeDirectory(path:string):Promise<void> {
  if(!isAbsolute(path)||resolve(path)!==path)throw new Error('Report paths must be absolute normalized directories');
  let current=parse(path).root;
  for(const part of path.slice(current.length).split(/[\\/]/).filter(Boolean)) {
    current=join(current,part);const stat=await lstat(current);
    if(!stat.isDirectory()||stat.isSymbolicLink())throw new Error('Symlinked or non-directory report path');
  }
}
async function generateAllureInternal(resultsDir:string,reportDir:string):Promise<{version:string;report_file:string}> {
  if(typeof resultsDir!=='string'||typeof reportDir!=='string')throw new Error('Invalid report paths');
  await safeDirectory(resultsDir);await safeDirectory(dirname(reportDir));
  if(!isAbsolute(reportDir)||resolve(reportDir)!==reportDir||reportDir===resultsDir||resultsDir.startsWith(reportDir+'/'))throw new Error('Invalid report output');
  let outputExists=true;
  try{await lstat(reportDir)}catch(error){if((error as NodeJS.ErrnoException).code==='ENOENT')outputExists=false;else throw error}
  if(outputExists)throw new Error('Report output already exists');
  const entries=await readdir(resultsDir,{withFileTypes:true});
  // 200 cases × (one result + six attachments), plus run metadata and lifecycle result.
  if(entries.length>200*7+2)throw new Error('Allure results exceed entry limit');
  let totalBytes=0;
  if(!entries.some(e=>e.name.endsWith('-result.json')))throw new Error('Allure results are empty');
  for(const entry of entries){
    const path=join(resultsDir,entry.name),stat=await lstat(path);
    if(stat.isSymbolicLink()||!stat.isFile()||stat.nlink!==1||stat.size>32*1024*1024)throw new Error('Unsupported Allure input entry');
    totalBytes+=stat.size;if(totalBytes>64*1024*1024)throw new Error('Allure input exceeds size limit');
    if(entry.name.endsWith('-result.json')){
      // Only Allure's attachment references are filesystem paths. Frozen business
      // JSON and parameters can legitimately contain arbitrary `source` values.
      const validateSteps=(value:unknown):void=>{
        if(!value||typeof value!=='object'||Array.isArray(value))throw new Error('Invalid Allure step');
        const step=value as Record<string,unknown>;
        if(step.attachments!==undefined){
          if(!Array.isArray(step.attachments))throw new Error('Invalid Allure attachments');
          for(const attachment of step.attachments){
            const source=attachment&&typeof attachment==='object'?attachment.source:undefined;
            if(typeof source!=='string'||!/^[a-zA-Z0-9_.-]+$/.test(source)||source.startsWith('.'))throw new Error('Invalid attachment source');
          }
        }
        if(step.steps!==undefined){
          if(!Array.isArray(step.steps))throw new Error('Invalid Allure steps');
          step.steps.forEach(validateSteps);
        }
      };
      validateSteps(JSON.parse(await readFile(path,'utf8')));
    }
  }
  const staging=await mkdtemp(join(dirname(reportDir),'.allure-stage-'));
  let reservedOutput=false;
  try {
  // Only application-owned runs receive a cleanup identity. Generic Allure inputs
  // remain supported, but any interrupted unmarked output needs manual review.
  let ownership:string|undefined;
  try {
    const run=JSON.parse(await readFile(join(resultsDir,'qinaegis-run.json'),'utf8'));
    const identity=(value:unknown):value is string=>typeof value==='string'&&/^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,127}$/.test(value);
    if(run.schema_version===1&&identity(run.project)&&identity(run.run_id)&&['completed','failed','stopped'].includes(run.status)) {
      ownership=JSON.stringify({schema_version:1,kind:'allure_report_stage',project:run.project,run_id:run.run_id,stage:basename(staging)});
      await writeFile(join(staging,'.qinaegis-report-stage.json'),ownership,{mode:0o600,flag:'wx'});
    }
  } catch(error) { if((error as NodeJS.ErrnoException).code!=='ENOENT')throw error; }
  const require=createRequire(import.meta.url),allureEntry=require.resolve('allure');
  const core=pathToFileURL(createRequire(allureEntry).resolve('@allurereport/core')).href;
  const pkg=JSON.parse(await readFile(join(dirname(dirname(allureEntry)),'package.json'),'utf8'));
  const worker=new Worker(`const {workerData,parentPort}=require('node:worker_threads');
    (async()=>{
      globalThis.fetch=async()=>{throw new Error('Network disabled during report generation')};
      const {AllureReport,resolveConfig}=await import(workerData.core);
      const config=await resolveConfig({name:'QinAegis execution report',output:workerData.output,
        plugins:{awesome:{options:{singleFile:true}}}},{cwd:workerData.output});
      const report=new AllureReport(config);await report.start();await report.readDirectory(workerData.input);await report.done();parentPort.postMessage({ok:true});
    })().catch(()=>parentPort.postMessage({ok:false}));`,{eval:true,execArgv:[],workerData:{core,input:await realpath(resultsDir),output:await realpath(staging)},stdout:true,stderr:true});
  worker.stdout?.on('data',()=>{});worker.stderr?.on('data',()=>{});
  await new Promise<void>((accept,reject)=>{
    let settled=false;
    const finish=async(ok:boolean)=>{
      if(settled)return;settled=true;clearTimeout(timer);
      await worker.terminate();ok?accept():reject(new Error('Allure generation failed'));
    };
    const timer=setTimeout(()=>{void finish(false)},90000);
    worker.once('message',message=>finish(message?.ok===true));worker.once('error',()=>finish(false));worker.once('exit',code=>{if(code!==0)finish(false)});
  });
  const index=join(staging,'index.html'),stat=await lstat(index);
  if(!stat.isFile()||stat.isSymbolicLink()||stat.size===0||stat.size>64*1024*1024)throw new Error('Allure report artifact unavailable');
  const html=offlineHtml(await readFile(index,'utf8'));
  if(Buffer.byteLength(html)>64*1024*1024)throw new Error('Allure report artifact exceeds size limit');
  await writeFile(index,html,{mode:0o600});
  await writeFile(join(staging,'qinaegis-reporter.json'),JSON.stringify({schema_version:1,reporter:'allure',version:pkg.version,mode:'awesome-single-file'}),{mode:0o600});
  for(const entry of await readdir(staging)){const path=join(staging,entry);const info=await lstat(path);if(!info.isFile()||info.isSymbolicLink())throw new Error('Unexpected report artifact');await chmod(path,0o600)}
  // Reserve the destination exclusively; publish index last so readers never see a partial HTML.
  await mkdir(reportDir,{mode:0o700});reservedOutput=true;
  if(ownership)await writeFile(join(reportDir,'.qinaegis-report-stage.json'),ownership,{mode:0o600,flag:'wx'});
  for(const entry of (await readdir(staging)).filter(name=>name!=='index.html'&&name!=='.qinaegis-report-stage.json'))await rename(join(staging,entry),join(reportDir,entry));
  await rename(index,join(reportDir,'index.html'));
  if(ownership)await rm(join(reportDir,'.qinaegis-report-stage.json'));
  return{version:pkg.version,report_file:'index.html'};
  } catch(error) {
    if(reservedOutput)await rm(reportDir,{recursive:true,force:true});
    throw error;
  } finally {await rm(staging,{recursive:true,force:true})}
}

export async function generateAllure(resultsDir:string,reportDir:string):Promise<{version:string;report_file:string}>{
  try{return await generateAllureInternal(resultsDir,reportDir)}catch{throw new Error('Allure generation failed; verify runtime and result assets')}
}
