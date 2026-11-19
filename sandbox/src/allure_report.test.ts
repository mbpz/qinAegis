import {test} from 'node:test';import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile,readFile,rm,realpath,symlink,lstat,readdir} from 'node:fs/promises';import {tmpdir}from'node:os';import {join}from'node:path';
import {generateAllure}from './allure_report.js';
test('owned interrupted-report identity is present during generation and removed on successful publish',async()=>{
 const root=await realpath(await mkdtemp(join(tmpdir(),'qinaegis-allure-ownership-')));
 try {
  const results=join(root,'allure-results'),report=join(root,'allure-report');await mkdir(results);
  await writeFile(join(results,'qinaegis-run.json'),JSON.stringify({schema_version:1,project:'project',run_id:'run-1',status:'failed'}));
  await writeFile(join(results,'case-result.json'),JSON.stringify({uuid:'owned',name:'Owned report fixture',status:'passed',stage:'finished',steps:[],attachments:[]}));
  const generation=generateAllure(results,report);
  let owner:Record<string,unknown>|undefined;
  for(let attempt=0;attempt<200&&!owner;attempt++){
   for(const stage of (await readdir(root)).filter(name=>name.startsWith('.allure-stage-'))){
    try{owner=JSON.parse(await readFile(join(root,stage,'.qinaegis-report-stage.json'),'utf8'));assert.equal(owner?.stage,stage);}catch(error){if((error as NodeJS.ErrnoException).code!=='ENOENT')throw error;}
   }
   if(!owner)await new Promise(accept=>setTimeout(accept,5));
  }
  assert.ok(owner,'owned staging marker was not observable before report publication');
  assert.equal(owner.schema_version,1);assert.equal(owner.kind,'allure_report_stage');assert.equal(owner.project,'project');assert.equal(owner.run_id,'run-1');
  await generation;
  await assert.rejects(lstat(join(report,'.qinaegis-report-stage.json')),{code:'ENOENT'});
  assert.equal((await readdir(root)).some(name=>name.startsWith('.allure-stage-')),false);
  assert.ok((await lstat(join(report,'index.html'))).isFile());
 }finally{await rm(root,{recursive:true,force:true})}
});
test('official pinned Allure produces standalone report and refuses overwrite or unsafe input',async()=>{
 const root=await realpath(await mkdtemp(join(tmpdir(),'qinaegis-allure-')));
 try{
  const results=join(root,'results'),report=join(root,'report');await mkdir(results);
  for(const status of ['passed','failed'])await writeFile(join(results,status+'-result.json'),JSON.stringify({uuid:status,name:'Receipt '+status,fullName:'receipt.'+status,status,stage:'finished',start:1,stop:20,statusDetails:status==='failed'?{message:'Receipt mismatch'}:{}}));
  assert.deepEqual(await generateAllure(results,report),{version:'3.33.0',report_file:'index.html'});
  const html=await readFile(join(report,'index.html'),'utf8');assert.match(html,/allure-single-file-assets/);assert.ok(html.length>10000);
  assert.match(html,/<head><meta http-equiv="Content-Security-Policy"/);
  assert.match(html,/connect-src blob: data:/);assert.match(html,/worker-src blob:/);
  assert.doesNotMatch(html,/googletagmanager|function gtag\(\)|const baseEl =|<base\b/);
  assert.match(html,/try\{void window\[key\]\}catch/);
  if(process.env.ALLURE_BROWSER_TEST==='1'){
   const {chromium}=await import('playwright');const browser=await chromium.launch({channel:'chrome',headless:true});
   try{const page=await browser.newPage();const errors:string[]=[];const external:string[]=[];
    page.on('pageerror',error=>errors.push(error.message));page.on('console',message=>{if(message.type()==='error')errors.push(message.text())});
    page.on('request',request=>{if(/^https?:/.test(request.url()))external.push(request.url())});
    await page.setContent('<iframe sandbox="allow-scripts" style="width:100%;height:1000px"></iframe>');
    await page.locator('iframe').evaluate((frame,value)=>{(frame as HTMLIFrameElement).srcdoc=value},html);
    await page.frameLocator('iframe').getByText('Receipt passed',{exact:true}).first().waitFor({timeout:30000});
    await page.frameLocator('iframe').getByText('Receipt failed',{exact:true}).first().waitFor();
    await page.waitForTimeout(1000);assert.deepEqual(errors,[]);assert.deepEqual(external,[]);
   }finally{await browser.close()}
  }
  assert.deepEqual(JSON.parse(await readFile(join(report,'qinaegis-reporter.json'),'utf8')),{schema_version:1,reporter:'allure',version:'3.33.0',mode:'awesome-single-file'});
  const summary=JSON.parse(await readFile(join(report,'summary.json'),'utf8'));assert.ok(summary);
  await assert.rejects(generateAllure(results,report));assert.equal(await readFile(join(report,'index.html'),'utf8'),html);
  await assert.rejects(generateAllure('relative',join(root,'relative')));
  if(process.platform !== 'win32'){await symlink(results,join(root,'linked'));await assert.rejects(generateAllure(join(root,'linked'),join(root,'linked-report')))}
  await writeFile(join(results,'unsafe-result.json'),JSON.stringify({attachments:[{source:'../private'}]}));await assert.rejects(generateAllure(results,join(root,'unsafe')));
  await assert.rejects(lstat(join(root,'unsafe')),{code:'ENOENT'});
  for(const source of ['/private/secret','https://example.test/private','../private','nested/file']){
   await writeFile(join(results,'unsafe-result.json'),JSON.stringify({steps:[{steps:[{attachments:[{source}]}]}]}));
   await assert.rejects(generateAllure(results,join(root,'unsafe')));
  }
  if(process.platform!=='win32'){
   await symlink(join(results,'passed-result.json'),join(results,'linked-attachment.json'));
   await writeFile(join(results,'unsafe-result.json'),JSON.stringify({attachments:[{source:'linked-attachment.json'}]}));
   await assert.rejects(generateAllure(results,join(root,'unsafe')));
   await rm(join(results,'linked-attachment.json'));
  }
  await rm(join(results,'unsafe-result.json'));
  await generateAllure(results,join(root,'unsafe'));
  assert.ok((await lstat(join(root,'unsafe','index.html'))).isFile());
  await writeFile(join(root,'occupied'),'unchanged');await assert.rejects(generateAllure(results,join(root,'occupied')));
  assert.equal(await readFile(join(root,'occupied'),'utf8'),'unchanged');
  assert.deepEqual((await (await import('node:fs/promises')).readdir(root)).filter(name=>name.startsWith('.allure-stage-')),[]);
 }finally{await rm(root,{recursive:true,force:true})}
});

test('official Allure generates the 200-case maximum without interpreting business source values as paths',async()=>{
 const root=await realpath(await mkdtemp(join(tmpdir(),'qinaegis-allure-capacity-')));
 try{
  const results=join(root,'results'),report=join(root,'report');await mkdir(results);
  for(let index=0;index<200;index++){
   const attachments=[];
   for(let part=0;part<6;part++){
    const source=`case-${index}-${part}-attachment.json`;
    attachments.push({name:`Frozen business ${part}`,source,type:'application/json'});
    await writeFile(join(results,source),JSON.stringify({params:{source:'https://example.test/business'},source:{endpoint:'https://example.test/object'},attachments:[{source:'../business-value-not-a-file-reference'}]}));
   }
   await writeFile(join(results,`case-${index}-result.json`),JSON.stringify({uuid:`case-${index}`,name:`Case ${index}`,fullName:`capacity.${index}`,status:'passed',stage:'finished',start:1,stop:2,params:{source:'https://example.test/parameter'},source:{business:true},attachments,steps:[{name:'Business step',status:'passed',params:{source:{url:'https://example.test/step'}},attachments:[],steps:[]}]}));
  }
  await writeFile(join(results,'qinaegis-run.json'),JSON.stringify({source:'https://example.test/run'}));
  await writeFile(join(results,'execution-result.json'),JSON.stringify({uuid:'lifecycle',name:'Execution lifecycle',status:'passed',stage:'finished',start:1,stop:2,attachments:[],steps:[]}));
  assert.equal((await (await import('node:fs/promises')).readdir(results)).length,1402);
  assert.deepEqual(await generateAllure(results,report),{version:'3.33.0',report_file:'index.html'});
  assert.match(await readFile(join(report,'index.html'),'utf8'),/allure-single-file-assets/);
  const generated=JSON.parse(await readFile(join(report,'test-results.json'),'utf8'));
  assert.equal(Object.keys(generated.byId).length,201);
  await writeFile(join(results,'excess.json'),'{}');
  await assert.rejects(generateAllure(results,join(root,'excess')));
 }finally{await rm(root,{recursive:true,force:true})}
});
