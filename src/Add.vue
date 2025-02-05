<script setup lang="ts">
import { ref, onMounted } from "vue";
import { listen } from '@tauri-apps/api/event';
import { invoke } from "@tauri-apps/api/core";
// import { getCurrentWindow } from '@tauri-apps/api/window';
import { toast } from 'vue3-toastify';
import 'vue3-toastify/dist/index.css';


const url = ref('');
const type = ref('author');
const authorProgress = ref('');
const comicProgress = ref('');

onMounted(() => {
  listen('err-msg-add', (e: any) => {
    toast(`${e.payload}`, {
      position: toast.POSITION.TOP_CENTER,
      type: 'error',
      autoClose: 2000,
    });
  });

  listen('author-progress', (e: any) => {
    authorProgress.value = e.payload;
  });
  listen('comic-progress', (e: any) => {
    comicProgress.value = e.payload;
  });
});

function urlChange(e: Event) {
  const target = e.target as HTMLTextAreaElement;
  let temp_url = target.value.trim();
  url.value = temp_url;
  let url_query: any = parseUrlParams(temp_url);

  if (url_query.zjid) {
    type.value = 'current';
  } else if (url_query.kuid) {
    type.value = 'juan_hua_fanwai';
  } else if (url_query.zz_name) {
    type.value = 'author';
  }
}

function typeChange(e: Event) {
  const target = e.target as HTMLSelectElement;
  type.value = target.value;
}

async function confirm() {
  if (url.value) {
    await invoke('add_new_task', {
      url: url.value,
      dlType: type.value,
    });
  } else {
    toast("Please input url!", {
      position: toast.POSITION.TOP_CENTER,
      type: 'error',
      autoClose: 2000,
    });
  }
}

function parseUrlParams(url: string) {
  const params: any = {};
  const urlObj = new URL(url);
  const searchParams = new URLSearchParams(urlObj.search);

  for (const [key, value] of searchParams.entries()) {
    params[key] = value;
  }

  return params;
}

</script>

<template>
  <div class="add-box">
    <textarea class="input" name="url" :value="url" @change="(e: any) => urlChange(e)" spellcheck="false"></textarea>
    <div class="tool">
      <div class="left">
        <select name="type" class="select" :value="type" @change="(e: any) => typeChange(e)">
          <option value="author">author</option>
          <option value="current">current</option>
          <option value="juan_hua_fanwai">juan_hua_fanwai</option>
          <option value="juan">juan</option>
          <option value="hua">hua</option>
          <option value="fanwai">fanwai</option>
        </select>
      </div>
      <div class="center" v-text="`author: ${authorProgress} comic: ${comicProgress}`"></div>
      <div class="right">
        <div class="start" @click="confirm">confirm</div>
      </div>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.add-box {
  width: 100%;
  height: 100%;
  padding: 10px 20px;

  .input {
    width: 100%;
    height: 120px;
    border: 1px solid #d9d9d9;
    outline: none;
    resize: none;
    padding: 10px 10px;
  }

  .tool {
    margin-top: 10px;
    display: flex;
    justify-content: space-between;
    align-items: center;

    .left {
      .select {
        font-size: 12px;
        outline: none;
        border-bottom: 1px solid #d9d9d9;
      }
    }

    .center {
      font-size: 12px;
      flex: 1;
      text-align: center;
    }

    .right {
      .start {
        cursor: pointer;
        font-size: 12px;

        &:hover {
          color: #4872ac;
        }
      }
    }
  }
}
</style>
