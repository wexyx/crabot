<script setup>
import {ref,watch} from 'vue'
const props=defineProps({values:{type:Object,required:true}})
const rows=ref([])
const error=ref('')
let emitted
watch(()=>props.values.AGENT_ENV_JSON,raw=>{
 if(raw===emitted)return
 try{rows.value=Object.entries(JSON.parse(raw||'{}')).map(([name,value])=>({name,value:value??'',stored:value===null,changed:false}))}catch{rows.value=[]}
},{immediate:true})
function publish(){
 error.value=''
 const result={}
 for(const row of rows.value){
  if(!row.name&&!row.value)continue
  if(!/^[A-Za-z_][A-Za-z0-9_]*$/.test(row.name)||Object.hasOwn(result,row.name)){error.value='名称必须合法且不能重复';break}
  result[row.name]=row.stored&&!row.changed?null:row.value
 }
 if(error.value){emitted='["invalid environment rows"]';props.values.AGENT_ENV_JSON=emitted;return}
 emitted=JSON.stringify(result);props.values.AGENT_ENV_JSON=emitted
}
function add(){rows.value.push({name:'',value:'',stored:false,changed:false})}
function remove(index){rows.value.splice(index,1);publish()}
</script>
<template>
 <section class="environment-fields">
  <div class="heading"><span>环境变量</span><el-button size="small" native-type="button" @click="add">新增自定义变量</el-button></div>
  <el-table v-if="rows.length" :data="rows" size="small" class="environment-table" scrollbar-always-on>
   <el-table-column label="变量名（自定义）" min-width="210"><template #default="{row}"><el-input v-model="row.name" :readonly="row.stored" placeholder="自由填写，如 MY_API_KEY" aria-label="环境变量名称" @input="publish"/></template></el-table-column>
   <el-table-column label="值" min-width="240"><template #default="{row}"><el-input v-model="row.value" type="password" show-password autocomplete="new-password" :placeholder="row.stored?'已设置；不修改则保留':'填写变量值'" aria-label="环境变量值" @input="row.changed=true;publish()"/></template></el-table-column>
   <el-table-column width="72" fixed="right"><template #default="{$index}"><el-button text type="danger" @click="remove($index)">删除</el-button></template></el-table-column>
  </el-table>
  <small v-if="error" role="alert">{{error}}</small>
  <small>仅当前 Agent 及其工具进程使用；不会修改全局环境。已保存的值不回显，本地明文保存。HOME、临时目录与 CRABOT_* 等运行目录变量不可覆盖。</small>
 </section>
</template>
<style scoped>
.environment-fields{grid-column:1/-1;min-width:0;display:grid;gap:10px}.heading{display:flex;flex-wrap:wrap;gap:8px;align-items:center;justify-content:space-between;font-size:14px}.environment-table{width:100%;min-width:0;max-width:100%}.environment-table :deep(.el-input){width:100%;min-width:0}.environment-table :deep(.el-input__wrapper){min-width:0}.environment-fields small{color:var(--muted);line-height:1.6}
</style>
