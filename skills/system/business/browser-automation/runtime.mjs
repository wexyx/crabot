import {spawn} from 'node:child_process'
import {lstat,mkdir,readFile,rename,rm,writeFile} from 'node:fs/promises'
import {randomUUID} from 'node:crypto'
import {homedir} from 'node:os'
import {join,resolve} from 'node:path'

export const puppeteerVersion='25.12.0'
export function runtimeDirectory(env=process.env){
 const alias=env.CRABOT_INSTANCE||''
 if(alias&&!/^[a-zA-Z0-9_-]{1,64}$/.test(alias))throw Error('Invalid Crabot instance name')
 const data=env.CRABOT_DATA_DIR||join(homedir(),alias?`.crabot_${alias}`:'.crabot')
 return join(resolve(data),'runtime','browser-automation','.runtime')
}
export async function runtimeReady(runtime=runtimeDirectory()){
 try{
  const manifest=JSON.parse(await readFile(join(runtime,'node_modules/puppeteer/package.json'),'utf8'))
  if(manifest.version!==puppeteerVersion)return false
  return await new Promise(resolve=>{
   const child=spawn(process.execPath,['--input-type=module','-e',checkScript(runtime)],{env:{...process.env,PUPPETEER_CACHE_DIR:join(runtime,'browsers')},stdio:'ignore'})
   child.once('error',()=>resolve(false));child.once('exit',code=>resolve(code===0))
  })
 }catch{return false}
}
function checkScript(runtime){
 return `import {createRequire} from 'node:module';import {pathToFileURL} from 'node:url';import {access} from 'node:fs/promises';import {constants} from 'node:fs';process.env.PUPPETEER_CACHE_DIR=${JSON.stringify(join(runtime,'browsers'))};const require=createRequire(${JSON.stringify(join(runtime,'package.json'))});const {default:p}=await import(pathToFileURL(require.resolve('puppeteer')).href);await access(await p.executablePath({headless:'shell'}),constants.X_OK)`
}
async function run(binary,args,env,cwd){
 await new Promise((resolve,reject)=>{
  const child=spawn(binary,args,{cwd,env,stdio:['ignore','pipe','pipe']})
  child.stdout.pipe(process.stderr,{end:false});child.stderr.pipe(process.stderr,{end:false})
  child.once('error',reject);child.once('exit',code=>code===0?resolve():reject(Error(`${binary} exited ${code}`)))
 })
}
async function lock(parent){
 const path=join(parent,'.install-lock')
 const token=randomUUID(),candidate=join(parent,`.install-candidate-${token}`)
 await mkdir(candidate)
 await writeFile(join(candidate,'owner.json'),JSON.stringify({pid:process.pid,token}))
 try{for(;;){
  try{await rename(candidate,path);return path}
  catch(e){if(!['EEXIST','ENOTEMPTY','EACCES'].includes(e.code))throw e}
  try{
   const owner=JSON.parse(await readFile(join(path,'owner.json'),'utf8'))
   if(!Number.isInteger(owner.pid)||owner.pid<=0||!/^[a-f0-9-]{36}$/.test(owner.token))throw Error('Invalid browser installation lock')
   try{process.kill(owner.pid,0)}catch(e){if(e.code==='ESRCH'){
    // Retain the tiny, nonempty tombstone: competing reclaimers cannot rename
    // a newly acquired lock over it after recovering the same dead owner.
    await rename(path,join(parent,`.interrupted-install-${owner.token}`)).catch(e=>{if(!['ENOENT','EEXIST','ENOTEMPTY'].includes(e.code))throw e})
   }else if(e.code!=='EPERM')throw e}
  }catch(e){if(e.code!=='ENOENT')throw e}
  await new Promise(resolve=>setTimeout(resolve,200))
 }}finally{await rm(candidate,{recursive:true,force:true})}
}
export async function ensureRuntime(){
 const runtime=runtimeDirectory()
 if(await runtimeReady(runtime))return runtime
 const data=resolve(runtime,'../../..')
 await mkdir(data,{recursive:true})
 for(const path of [join(data,'runtime'),join(data,'runtime','browser-automation'),runtime]){
  await mkdir(path,{recursive:true})
  if(!(await lstat(path)).isDirectory())throw Error(`Runtime directory must not be a symlink: ${path}`)
 }
 const held=await lock(join(runtime,'..'))
 try{
  if(await runtimeReady(runtime))return runtime
  const env={...process.env,PUPPETEER_CACHE_DIR:join(runtime,'browsers'),PUPPETEER_SKIP_DOWNLOAD:'true',npm_config_cache:join(runtime,'.npm-cache')}
  await run('npm',['install','--prefix',runtime,'--ignore-scripts','--no-audit','--no-fund','--save-exact',`puppeteer@${puppeteerVersion}`],env,runtime)
  delete env.PUPPETEER_SKIP_DOWNLOAD
  const packageDir=join(runtime,'node_modules/puppeteer')
  const manifest=JSON.parse(await readFile(join(packageDir,'package.json'),'utf8'))
  await run(process.execPath,[join(packageDir,typeof manifest.bin==='string'?manifest.bin:manifest.bin.puppeteer),'browsers','install','chrome-headless-shell'],env,runtime)
  // Check in a fresh process so failed pre-install imports cannot poison module caches.
  await run(process.execPath,['--input-type=module','-e',checkScript(runtime)],env,runtime)
  return runtime
 }finally{await rm(held,{recursive:true,force:true})}
}
