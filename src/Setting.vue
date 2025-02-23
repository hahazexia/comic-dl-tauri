<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { onMounted, ref } from "vue";
import { toast } from 'vue3-toastify';
import 'vue3-toastify/dist/index.css';

const download_dir = ref('');
const concurrent_task = ref('1');
const concurrent_img = ref('10');
const img_timeout = ref('5');
const img_retry_count = ref('3');
const download_dir_flag = ref(false);

async function submit() {
  await invoke('setting_save', {
    downloadDir: download_dir.value,
    concurrentTask: concurrent_task.value,
    concurrentImg: concurrent_img.value,
    imgTimeout: img_timeout.value,
    imgRetryCount: img_retry_count.value,
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
  download_dir.value = res;
  download_dir_flag.value = false;
}

const handleInput = (inputRef: any) => {
  let value = inputRef.value.replace(/\D/g, '');
  value = value.replace(/^0+/, '');
  inputRef.value = value;
};

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
    img_timeout.value = res.img_timeout;
    img_retry_count.value = res.img_retry_count;
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
          v-model="concurrent_task" @input="() => handleInput(concurrent_task)">
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

      <div class="form-item">
        <label for="img_timeout">img timeout<span>:</span></label>
        <input class="form-input task-input" name="img_timeout" id="img_timeout" type="text" spellcheck="false"
          v-model="img_timeout" @input="() => handleInput(img_timeout)">
      </div>

      <div class="form-item">
        <label for="img_retry_count">img retry count<span>:</span></label>
        <input class="form-input task-input" name="img_retry_count" id="img_retry_count" type="text" spellcheck="false"
          v-model="img_retry_count" @input="() => handleInput(img_retry_count)">
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
