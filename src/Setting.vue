<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { onMounted, ref } from "vue";
import { toast } from 'vue3-toastify';
import 'vue3-toastify/dist/index.css';

const download_dir = ref('');
const concurrent_task = ref('1');
const concurrent_img = ref('10');
const download_dir_flag = ref(false);

async function submit() {
  await invoke('setting_save', {
    downloadDir: download_dir.value,
    concurrentTask: concurrent_task.value,
    concurrentImg: concurrent_img.value,
  });
}

async function downloadDir() {
  if (download_dir_flag.value) {
    return;
  }
  download_dir_flag.value = true;
  let res: string = await invoke('download_dir', {
    currentDir: download_dir.value,
  });
  console.log(res, '目录看看');
  download_dir.value = res;
  download_dir_flag.value = false;
}

function concurrentTaskInput() {
  let value = concurrent_task.value.replace(/\D/g, '');
  value = value.replace(/^0+/, '');
  concurrent_task.value = value;
}

onMounted(() => {
  listen('err_msg_setting', (e: any) => {
    toast(`${e.payload}`, {
      position: toast.POSITION.TOP_CENTER,
      type: 'error',
      autoClose: 2000,
    });
  });
  (async () => {
    let res: any = await invoke('get_setting');

    download_dir.value = res.download_dir;
    concurrent_task.value = res.concurrent_task;
    concurrent_img.value = res.concurrent_img;
  })();
});

</script>

<template>
  <div class="setting">
    <h5>Setting</h5>
    <form action="#">
      <div class="form-item">
        <label for="download_dir">download dir<span>:</span></label>
        <input class="form-input" name="download_dir" id="download_dir" type="text" spellcheck="false"
          v-model="download_dir">
        <button class="form-btn" @click.prevent="downloadDir">•••</button>
      </div>
      <div class="form-item">
        <label for="concurrent_task">concurrent task<span>:</span></label>
        <input class="form-input task-input" name="concurrent_task" id="concurrent_task" type="text" spellcheck="false"
          v-model="concurrent_task" @input="concurrentTaskInput">
      </div>
      <div class="form-item">
        <label for="concurrent_img">concurrent img<span>:</span></label>
        <select class="form-select" name="concurrent_img" id="concurrent_img" v-model="concurrent_img">
          <option value="5">5</option>
          <option value="10">10</option>
          <option value="15">15</option>
          <option value="20">20</option>
          <option value="25">25</option>
          <option value="30">30</option>
        </select>
      </div>
      <div class="btns">
        <button class="submit" @click.prevent="submit">submit</button>
      </div>
    </form>
  </div>
</template>

<style lang="scss" scoped>
.setting {
  padding: 20px;

  .form-item {
    padding: 10px 0;
    display: flex;
    justify-content: flex-start;

    label {
      display: flex;
      justify-content: space-between;
      font-size: 12px;
      width: 130px;
      color: #166d67;
      font-weight: 200;
    }

    .form-input {
      font-size: 12px;
      outline: none;
      border-bottom: 1px solid #d9d9d9;
      width: 360px;
    }

    .task-input {
      width: 100px;
    }

    .form-select {
      width: 100px;
      font-size: 12px;
      outline: none;
      border-bottom: 1px solid #d9d9d9;
    }
  }

  .btns {
    text-align: right;

    .submit {
      color: purple;
      border-bottom: 1px solid #d9d9d9;
      font-size: 12px;

      &:hover {
        filter: hue-rotate(180deg) brightness(0.8) saturate(2);
      }
    }
  }
}
</style>
