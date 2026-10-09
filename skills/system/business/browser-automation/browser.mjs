import {createRequire} from 'node:module'
import {mkdtemp,mkdir,lstat,rm} from 'node:fs/promises'
import {basename,dirname,join,resolve} from 'node:path'
import {fileURLToPath,pathToFileURL} from 'node:url'
import {ensureRuntime,runtimeDirectory} from './runtime.mjs'

const runtime=runtimeDirectory()
process.env.PUPPETEER_CACHE_DIR=join(runtime,'browsers')
const require=createRequire(join(runtime,'package.json'))
async function temporaryRoot(){
 const alias=process.env.CRABOT_INSTANCE||''
 if(alias&&!/^[a-zA-Z0-9_-]{1,64}$/.test(alias))throw Error('Invalid Crabot instance name')
 const root=process.env.CRABOT_TMP_DIR||join(process.cwd(),alias?'.crabot_'+alias:'.crabot','tmp')
 if(!process.env.CRABOT_TMP_DIR){
  const parent=dirname(root)
  await mkdir(parent,{recursive:true})
  if((await lstat(parent)).isSymbolicLink())throw Error('Temporary directory cannot be a symlink')
 }
 await mkdir(root,{recursive:true})
 if((await lstat(root)).isSymbolicLink())throw Error('Temporary directory cannot be a symlink')
 return root
}
export async function withBrowser(work){
 await ensureRuntime()
 let puppeteer
 try{puppeteer=(await import(pathToFileURL(require.resolve('puppeteer')).href)).default}
 catch{throw Error('请先运行此 Skill 的 node install.mjs 安装 Puppeteer 和浏览器。')}
 const profile=await mkdtemp(join(await temporaryRoot(),'crabot-browser-'))
 let browser
 try{
  browser=await puppeteer.launch({headless:'shell',userDataDir:profile,env:{...process.env,MAC_CHROMIUM_TMPDIR:profile},args:['--disable-gpu']})
  const page=await browser.newPage()
  page.setDefaultTimeout(30000)
  page.setDefaultNavigationTimeout(60000)
  return await work(page,browser)
 }finally{try{await browser?.close()}finally{await rm(profile,{recursive:true,force:true})}}
}
if(process.argv[1]&&resolve(process.argv[1])===fileURLToPath(import.meta.url)){
 const [url,output]=process.argv.slice(2)
 const address=url?new URL(url):null
 if(!address||!['http:','https:'].includes(address.protocol)||address.username||address.password)throw Error('用法：node browser.mjs HTTP_URL [截图路径]，URL 不可包含凭据')
 const outputDir=output?await mkdtemp(join(await temporaryRoot(),'browser-output-')):null
 const target=outputDir?join(outputDir,basename(output)):null
 await withBrowser(async page=>{
  await page.goto(url,{waitUntil:'domcontentloaded'})
  if(output)await page.screenshot({path:target,fullPage:true})
  console.log(JSON.stringify({url:page.url(),title:await page.title(),text:(await page.locator('body').map(el=>el.innerText).wait()).slice(0,16000),screenshot:target}))
 })
}
