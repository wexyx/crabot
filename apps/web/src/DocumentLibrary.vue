<script setup>
import {ref,onMounted,onBeforeUnmount} from 'vue'
import ConfigPanel from './ConfigPanel.vue'
import {ElMessage,ElMessageBox} from 'element-plus'
const props=defineProps({request:Function})
defineEmits(['close','switch'])
const editor=ref(false),rows=ref([]),query=ref(''),activeQuery=ref(''),url=ref(''),depth=ref(0),maxPages=ref(20),ignoredParams=ref(''),report=ref(null),busy=ref(false),error=ref(''),page=ref(1),pageSize=ref(20),total=ref(0),fileInput=ref(),draft=ref({title:'',source:'',content:''})
let epoch=0
const base=()=>'/v1/documents'
const action=body=>props.request(base(),{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify(body)})
async function load(target=1,size=pageSize.value,search=activeQuery.value){
 const generation=++epoch
 const data=await action({action:'search',query:search,offset:(target-1)*size,limit:size})
 if(generation!==epoch)return
 const last=Math.max(1,Math.ceil(data.total/size))
 if(target>last)return load(last,size,search)
 rows.value=data.hits;total.value=data.total;page.value=target;pageSize.value=size;activeQuery.value=search
}
const search=()=>load(1,pageSize.value,query.value.trim())
const importedAt=value=>value?new Date(Number(value)).toLocaleString('zh-CN',{hour12:false}):'—'
async function perform(fn){if(busy.value)return;busy.value=true;error.value='';try{await fn()}catch(e){error.value=e.message}finally{busy.value=false}}
async function importUrl(){report.value=await action({action:'import',url:url.value,depth:depth.value,max_pages:maxPages.value,ignored_query_params:ignoredParams.value.split(',').map(s=>s.trim()).filter(Boolean)});await load();ElMessage.success('导入任务已结束，请查看结果')}
async function upload(event){const file=event.target.files?.[0];event.target.value='';if(!file)return;await perform(async()=>{if(file.size>20*1024*1024)throw new Error('文件不能超过 20 MiB');const result=await props.request(`${base()}/upload?name=${encodeURIComponent(file.name)}`,{method:'POST',headers:{'content-type':'application/octet-stream'},body:file});await load();ElMessage.success(result.already_exists?'同名来源已存在，请编辑现有文档':'文档已入库')})}
async function edit(row){const generation=epoch;let start=0,parts=[],data;do{data=await action({action:'read',id:row.id,offset:start,limit:100});if(generation!==epoch)return;parts.push(...data.chunks.map(c=>c.text));start=data.next_offset}while(data.truncated);draft.value={id:row.id,expected_version:data.version,title:data.title,source:data.source,content:parts.join('')};editor.value=true}
async function save(){await action({action:'save',...draft.value});editor.value=false;await load(draft.value.id?page.value:1);ElMessage.success('文档已保存')}
async function remove(row){try{await ElMessageBox.confirm(`删除「${row.title}」及其索引？`,'删除文档',{type:'warning'})}catch{return}await perform(async()=>{await action({action:'delete',id:row.id,expected_version:row.version});await load(page.value)})}
onMounted(()=>perform(()=>load()))
onBeforeUnmount(()=>{epoch++})
</script>
<template>
 <ConfigPanel inline title="知识库" eyebrow="KNOWLEDGE" description="导入与维护可供 Agent 检索的文档。" :busy="busy" @close="$emit('close')">
  <el-tabs class="capability-tabs" model-value="doc" @tab-change="key=>$emit('switch',key==='tool'?'tools':key==='skill'?'skills':'docs')"><el-tab-pane label="工具" name="tool" :disabled="busy"/><el-tab-pane label="Skills" name="skill" :disabled="busy"/><el-tab-pane label="知识库" name="doc"/></el-tabs>
  <p class="hint">当前 Crabot 统一知识库 · 所有项目与本地 Agent 共享 · 网页 / PDF / Word（DOCX）/ Markdown / 纯文本</p>
  <div class="toolbar"><el-input v-model="query" :disabled="busy" clearable placeholder="搜索标题或正文；留空查看全部" @clear="perform(search)" @keyup.enter="perform(search)"/><el-button :loading="busy" @click="perform(search)">搜索</el-button><el-button :disabled="busy" @click="draft={title:'',source:'',content:''};editor=true">新建</el-button><el-button :disabled="busy" @click="fileInput.click()">上传文档</el-button><input ref="fileInput" type="file" accept=".pdf,.docx,.md,.txt,.html,.csv,.json" hidden @change="upload"/></div>
  <div class="toolbar"><el-input v-model="url" placeholder="https://… 网页或文档地址" @keyup.enter="perform(importUrl)"/><el-button type="primary" :disabled="!url.trim()" :loading="busy" @click="perform(importUrl)">从网址导入</el-button></div>
  <div class="crawl-options"><label>爬取深度 <el-input-number v-model="depth" :min="0" :max="5" size="small"/></label><label>最多页面 <el-input-number v-model="maxPages" :min="1" :max="100" size="small"/></label><el-input v-model="ignoredParams" placeholder="额外忽略的 query 参数，如 ref,source" size="small"/></div>
  <p class="hint">深度 0 仅当前页；只跟随同站链接。自动移除 utm_* 等跟踪参数，未知参数保留。扫描件需先 OCR；动态网页可由浏览器 Skill 提取后保存。</p>
  <el-collapse v-if="report"><el-collapse-item :title="`导入结果 · ${report.documents?.length||0} 页已处理 · ${report.errors?.length||0} 页失败`"><el-table :data="[...(report.documents||[]).map(d=>({...d,result:d.already_exists?'已存在':'已入库'})),...(report.errors||[]).map(e=>({source:e.url,result:e.error}))]"><el-table-column prop="source" label="地址" show-overflow-tooltip/><el-table-column prop="result" label="结果" show-overflow-tooltip/></el-table><p v-if="report.truncated">已达到页面上限，剩余链接未抓取。</p></el-collapse-item></el-collapse>
  <el-alert v-if="error" :title="error" type="error" :closable="false"/>
  <p class="list-summary">{{activeQuery?'搜索结果':'全部文档'}} · {{total}} 篇 · 按导入时间从新到旧</p>
  <el-table :data="rows" row-key="id" v-loading="busy" :empty-text="activeQuery?'暂无匹配文档':'暂无文档，导入一篇开始使用'"><el-table-column prop="title" label="文档" min-width="150" show-overflow-tooltip/><el-table-column prop="source" label="来源" min-width="190" show-overflow-tooltip/><el-table-column label="导入时间" width="175"><template #default="{row}">{{importedAt(row.created_at)}}</template></el-table-column><el-table-column prop="version" label="版本" width="65"/><el-table-column label="操作" width="140"><template #default="{row}"><el-button link type="primary" :disabled="busy" @click="perform(()=>edit(row))">查看 / 编辑</el-button><el-button link type="danger" :disabled="busy" @click="remove(row)">删除</el-button></template></el-table-column></el-table>
  <el-pagination class="document-pagination" :current-page="page" :page-size="pageSize" :page-sizes="[10,20,50,100]" :total="total" :disabled="busy" :pager-count="5" layout="total, sizes, prev, pager, next" @current-change="value=>perform(()=>load(value))" @size-change="value=>perform(()=>load(1,value))"/>
 </ConfigPanel>
 <el-drawer v-model="editor" :title="draft.id?'编辑文档':'新建文档'" size="min(760px,96vw)" append-to-body>
  <el-form label-position="top"><el-form-item label="标题"><el-input v-model="draft.title" maxlength="1024"/></el-form-item><el-form-item label="来源"><el-input v-model="draft.source" maxlength="4096"/></el-form-item><el-form-item label="正文"><el-input v-model="draft.content" type="textarea" :rows="20"/></el-form-item></el-form>
  <el-alert v-if="error" :title="error" type="error" :closable="false"/>
  <template #footer><el-button @click="editor=false">取消</el-button><el-button type="primary" :loading="busy" :disabled="!draft.title.trim()||!draft.content.trim()" @click="perform(save)">保存</el-button></template>
 </el-drawer>
</template>
<style scoped>.toolbar{display:flex;gap:8px;margin-bottom:14px}.hint{font-size:12px;color:var(--muted);margin-bottom:20px}.el-alert{margin:12px 0}.crawl-options{display:flex;flex-wrap:wrap;gap:16px;align-items:center}.crawl-options label{display:flex;gap:8px;align-items:center;white-space:nowrap}.crawl-options>.el-input{flex:1;min-width:220px}.list-summary{font-size:12px;color:var(--muted);margin:16px 0 8px}.document-pagination{margin-top:16px;flex-wrap:wrap;gap:8px}</style>
