<script setup>
import {ref,computed} from 'vue'
import {Folder,ArrowUp} from '@element-plus/icons-vue'
const props=defineProps({modelValue:String,request:Function})
const emit=defineEmits(['update:modelValue'])
const open=ref(false),busy=ref(false),error=ref(''),directory=ref(null),filter=ref('')
const rows=computed(()=>directory.value?.directories.filter(d=>d.name.toLowerCase().includes(filter.value.toLowerCase()))||[])
async function browse(path){
 if(busy.value)return
 busy.value=true;error.value=''
 try{directory.value=await props.request('/v1/workspace/directories'+(path?'?path='+encodeURIComponent(path):''));filter.value=''}
 catch(e){error.value=e.message}
 finally{busy.value=false}
}
function show(){open.value=true;directory.value=null;browse(props.modelValue)}
function select(){emit('update:modelValue',directory.value.path);open.value=false}
</script>
<template>
 <div class="directory-field"><el-input :model-value="modelValue" readonly placeholder="继承 Crabot 启动目录" aria-label="项目工作目录"/><el-button @click="show" :icon="Folder">选择文件夹</el-button><el-button v-if="modelValue" text @click="emit('update:modelValue','')">恢复默认</el-button></div>
 <el-dialog v-model="open" title="选择工作目录" width="min(620px,94vw)" append-to-body>
  <p class="directory-note">浏览 Crabot 所在机器的文件夹；选择目录不会立即执行任务或授予目录外访问权限。</p>
  <el-alert v-if="error" type="error" :title="error" :closable="false"/>
  <div class="directory-toolbar"><el-button :icon="ArrowUp" :disabled="busy||!directory?.parent" @click="browse(directory.parent)">上一级</el-button><el-button :disabled="busy" @click="browse()">启动目录</el-button></div>
  <p class="directory-path">{{directory?.path||'尚未选择'}}</p>
  <el-input v-model="filter" placeholder="筛选文件夹" clearable/>
  <div v-loading="busy" class="directory-list"><button type="button" v-for="row in rows" :key="row.path" @click="browse(row.path)" :disabled="busy"><el-icon><Folder/></el-icon>{{row.name}}<span>›</span></button><el-empty v-if="!busy&&!rows.length" description="没有匹配的子文件夹" :image-size="48"/></div>
  <template #footer><el-button @click="open=false">取消</el-button><el-button type="primary" :disabled="busy||!directory||!!error" @click="select">选择当前文件夹</el-button></template>
 </el-dialog>
</template>
<style scoped>
.directory-field{display:flex;gap:8px;align-items:center;flex-wrap:wrap}.directory-field>.el-input{flex:1;min-width:180px}.directory-note{color:var(--muted);font-size:12px}.directory-toolbar{display:flex;gap:8px;margin:12px 0}.directory-path{overflow-wrap:anywhere}.directory-list{height:300px;overflow:auto;margin-top:10px}.directory-list button{width:100%;display:flex;align-items:center;gap:10px;padding:12px;border:0;background:transparent;color:var(--text);text-align:left;cursor:pointer}.directory-list button:hover{background:var(--el-fill-color-light)}.directory-list button span{margin-left:auto}
</style>
