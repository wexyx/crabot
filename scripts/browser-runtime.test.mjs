import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp,mkdir,writeFile,readFile,rm,cp,access} from 'node:fs/promises'
import {tmpdir,homedir} from 'node:os'
import {join,resolve,dirname} from 'node:path'
import {spawn} from 'node:child_process'
import {runtimeDirectory,runtimeReady,puppeteerVersion} from '../skills/system/business/browser-automation/runtime.mjs'

const source=resolve('skills/system/business/browser-automation')
function run(script,env,args=[]){
 return new Promise((resolve,reject)=>{
  const child=spawn(process.execPath,[script,...args],{env:{...process.env,...env},stdio:['ignore','pipe','pipe']});let out='',err=''
  child.stdout.on('data',b=>out+=b);child.stderr.on('data',b=>err+=b)
  child.once('error',reject);child.once('exit',code=>code===0?resolve(out):reject(Error(err)))
 })
}
test('runtime directory follows instance data, not release, cwd or temporary HOME',()=>{
 assert.equal(runtimeDirectory({}),join(homedir(),'.crabot/runtime/browser-automation/.runtime'))
 assert.equal(runtimeDirectory({CRABOT_INSTANCE:'demo'}),join(homedir(),'.crabot_demo/runtime/browser-automation/.runtime'))
 assert.equal(runtimeDirectory({CRABOT_DATA_DIR:'/tmp/custom-instance',CRABOT_INSTANCE:'demo'}),'/tmp/custom-instance/runtime/browser-automation/.runtime')
 assert.throws(()=>runtimeDirectory({CRABOT_INSTANCE:'../bad'}))
})
test('readiness check resolves the installed browser when CRABOT_TMP_DIR is set',{timeout:30000},async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-browser-ready-'))
 try{
  const data=join(dir,'.crabot_ready')
  // Build a real runtime layout with a stub Puppeteer whose executablePath()
  // resolves from the cache directory, and a browser that exists on disk.
  const runtime=join(data,'runtime','browser-automation','.runtime')
  const pkg=join(runtime,'node_modules','puppeteer')
  await mkdir(pkg,{recursive:true})
  await writeFile(join(pkg,'package.json'),JSON.stringify({name:'puppeteer',version:puppeteerVersion,main:'index.cjs'}))
  await writeFile(join(pkg,'index.cjs'),
   "const p=require('path');exports.default=exports;exports.executablePath=async()=>p.join(process.env.PUPPETEER_CACHE_DIR,'chrome-headless-shell','mac_arm-154.0.8037.57','chrome-headless-shell')\n")
  const exe=join(runtime,'browsers','chrome-headless-shell','mac_arm-154.0.8037.57','chrome-headless-shell')
  await mkdir(dirname(exe),{recursive:true})
  await writeFile(exe,'fixture',{mode:0o700})
  // CRABOT_TMP_DIR must not hijack browser resolution: the readiness probe has
  // to point Puppeteer at the installed cache, not Puppeteer's default tmpDir.
  const ready=await runtimeReady(runtimeDirectory({CRABOT_DATA_DIR:data,CRABOT_TMP_DIR:join(dir,'elsewhere')}))
  assert.equal(ready,true)
  await rm(exe)
  const missing=await runtimeReady(runtimeDirectory({CRABOT_DATA_DIR:data,CRABOT_TMP_DIR:join(dir,'elsewhere')}))
  assert.equal(missing,false)
 }finally{await rm(dir,{recursive:true,force:true})}
})
test('build once, reuse across releases, repair missing browser/package, isolate instances and serialize installers',{timeout:30000},async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-browser-runtime-'))
 try{
  const bin=join(dir,'bin'),data=join(dir,'.crabot_test'),log=join(dir,'installs.log')
  await mkdir(bin)
  // Offline npm fixture installs a minimal Puppeteer module and browser installer.
  await writeFile(join(bin,'npm'),`#!${process.execPath}
const fs=require('fs'),p=require('path');const runtime=process.argv[process.argv.indexOf('--prefix')+1];
fs.appendFileSync(process.env.TEST_INSTALL_LOG,'install\\n');
const pkg=p.join(runtime,'node_modules/puppeteer');fs.mkdirSync(pkg,{recursive:true});
fs.writeFileSync(p.join(pkg,'package.json'),JSON.stringify({name:'puppeteer',version:'25.12.0',main:'index.cjs',bin:'cli.cjs'}));
fs.writeFileSync(p.join(pkg,'index.cjs'),"exports.executablePath=()=>require('path').join(process.env.PUPPETEER_CACHE_DIR,'headless-shell')");
fs.writeFileSync(p.join(pkg,'cli.cjs'),"const fs=require('fs'),p=require('path');fs.mkdirSync(process.env.PUPPETEER_CACHE_DIR,{recursive:true});fs.writeFileSync(p.join(process.env.PUPPETEER_CACHE_DIR,'headless-shell'),'fixture',{mode:0o700})");
`,{mode:0o700})
  const env={PATH:`${bin}:${process.env.PATH}`,CRABOT_DATA_DIR:data,HOME:join(dir,'scratch-home'),TEST_INSTALL_LOG:log}
  const release1=join(dir,'release1'),release2=join(dir,'release2')
  for(const release of [release1,release2]){
   await mkdir(release)
   for(const file of ['runtime.mjs','install.mjs'])await cp(join(source,file),join(release,file))
  }
  await Promise.all([run(join(release1,'install.mjs'),env),run(join(release2,'install.mjs'),env)])
  const count=async()=>(await readFile(log,'utf8')).trim().split('\n').length
  assert.equal(await count(),1)
  await run(join(release2,'install.mjs'),env);assert.equal(await count(),1)
  const runtime=runtimeDirectory(env)
  await assert.rejects(access(join(release1,'.runtime')))
  await rm(join(runtime,'browsers/headless-shell'))
  await run(join(release1,'install.mjs'),env);assert.equal(await count(),2)
  await rm(join(runtime,'node_modules'),{recursive:true})
  await run(join(release2,'install.mjs'),env);assert.equal(await count(),3)
  await run(join(release2,'install.mjs'),{...env,CRABOT_DATA_DIR:join(dir,'.crabot_other')});assert.equal(await count(),4)
  // A dead installer's lock is recovered automatically on the next build.
  await rm(join(runtime,'browsers/headless-shell'))
  const lock=join(runtime,'..','.install-lock');await mkdir(lock)
  await writeFile(join(lock,'owner.json'),JSON.stringify({pid:2147483647,token:'00000000-0000-0000-0000-000000000001'}))
  await run(join(release1,'install.mjs'),env);assert.equal(await count(),5)
 }finally{await rm(dir,{recursive:true,force:true})}
})

test('browser Skill reads and screenshots through its own script without a native browser tool',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-browser-script-'))
 try{
  const data=join(dir,'instance'),tmp=join(dir,'work','.crabot','tmp')
  await mkdir(tmp,{recursive:true})
  const runtime=runtimeDirectory({CRABOT_DATA_DIR:data}),pkg=join(runtime,'node_modules','puppeteer')
  await mkdir(pkg,{recursive:true})
  await writeFile(join(pkg,'package.json'),JSON.stringify({version:puppeteerVersion,main:'index.cjs'}))
  const exe=join(runtime,'browser');await writeFile(exe,'fixture',{mode:0o700})
  await writeFile(join(pkg,'index.cjs'),`const fs=require('fs/promises');exports.executablePath=()=>${JSON.stringify(exe)};exports.launch=async options=>{
    if(options.args.includes('--no-sandbox'))throw Error('sandbox must remain enabled');
    const page={setDefaultTimeout(){},setDefaultNavigationTimeout(){},goto:async()=>{},url:()=> 'https://example.com/',title:async()=> 'Example',
      locator:()=>({map:()=>({wait:async()=> 'Example page'})}),screenshot:async({path})=>fs.writeFile(path,Buffer.from([137,80,78,71,13,10,26,10]))};
    return {newPage:async()=>page,close:async()=>{}}
  }`)
  const env={CRABOT_DATA_DIR:data,CRABOT_TMP_DIR:tmp}
  const result=JSON.parse(await run(join(source,'browser.mjs'),env,['https://example.com','shot.png']))
  assert.equal(result.title,'Example');assert.equal(result.text,'Example page')
  assert.ok(result.screenshot.startsWith(tmp+'/browser-output-'))
  assert.equal((await readFile(result.screenshot)).length,8)
  await assert.rejects(run(join(source,'browser.mjs'),env,['file:///etc/passwd']))
  await assert.rejects(run(join(source,'browser.mjs'),env,['https://user:pass@example.com']))
 }finally{await rm(dir,{recursive:true,force:true})}
})
