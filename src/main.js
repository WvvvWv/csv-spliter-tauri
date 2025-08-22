// 使用正确的Tauri API导入
const { invoke } = window.__TAURI__.tauri || window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;
const { appDataDir } = window.__TAURI__.path;

// 全局变量
let csvFilePath = '';
let outputDir = '';

// DOM元素
const csvFileInput = document.getElementById('csv-file-path');
const outputDirInput = document.getElementById('output-dir');
const selectFileBtn = document.getElementById('select-file-btn');
const selectDirBtn = document.getElementById('select-dir-btn');
const splitBtn = document.getElementById('split-btn');
const hasHeaderCheckbox = document.getElementById('has-header');
const convertExcelCheckbox = document.getElementById('convert-excel');
const rowsPerFileInput = document.getElementById('rows-per-file');
const progressContainer = document.getElementById('progress-container');
const progressBar = document.getElementById('progress-bar');
const progressText = document.getElementById('progress-text');
const statusMessage = document.getElementById('status-message');

// 初始化
window.addEventListener('DOMContentLoaded', () => {
  console.log('DOM loaded, checking Tauri API...');
  console.log('Tauri available:', !!window.__TAURI__);
  if (window.__TAURI__) {
    console.log('Core API:', !!window.__TAURI__.core);
    console.log('Dialog API:', !!window.__TAURI__.dialog);
  }
  setupEventListeners();
  // 设置默认行数为500000
  if (rowsPerFileInput) {
    rowsPerFileInput.value = 500000;
  }
  updateSplitButtonState();
});

// 设置事件监听器
function setupEventListeners() {
  console.log('Setting up event listeners...');
  selectFileBtn.addEventListener('click', selectCsvFile);
    selectDirBtn.addEventListener('click', selectOutputDirectory);
    csvFileInput.addEventListener('click', selectCsvFile);
    outputDirInput.addEventListener('click', selectOutputDirectory);
    splitBtn.addEventListener('click', startCsvSplit);
  
  // 监听输入变化以更新按钮状态
  [csvFileInput, outputDirInput, rowsPerFileInput].forEach(input => {
    input.addEventListener('input', updateSplitButtonState);
  });
  
  console.log('Event listeners set up complete');
}

// 选择CSV文件
async function selectCsvFile() {
  console.log('selectCsvFile clicked');
  try {
    if (typeof window.__TAURI__ === 'undefined') {
      showStatus('请在Tauri环境中运行此应用', 'error');
      return;
    }
    
    console.log('Tauri available, opening file dialog...');
    const selected = await window.__TAURI__.dialog.open({
      filters: [{
        name: 'CSV文件',
        extensions: ['csv']
      }]
    });
    
    console.log('File selected:', selected);
    if (selected) {
      csvFilePath = Array.isArray(selected) ? selected[0] : selected;
      csvFileInput.value = csvFilePath;
      updateSplitButtonState();
      console.log('File path set:', csvFilePath);
    }
  } catch (error) {
    console.error('Error selecting file:', error);
    if (error && error.message) {
      showStatus('选择文件失败: ' + error.message, 'error');
    } else {
      showStatus('选择文件失败: ' + error, 'error');
    }
  }
}

// 选择输出目录
async function selectOutputDirectory() {
  console.log('selectOutputDirectory clicked');
  try {
    if (typeof window.__TAURI__ === 'undefined') {
      showStatus('请在Tauri环境中运行此应用', 'error');
      return;
    }
    
    console.log('Tauri available, opening directory dialog...');
    const selected = await window.__TAURI__.dialog.open({
      directory: true,
      multiple: false
    });
    
    console.log('Directory selected:', selected);
    if (selected) {
      outputDir = Array.isArray(selected) ? selected[0] : selected;
      outputDirInput.value = outputDir;
      updateSplitButtonState();
      console.log('Directory path set:', outputDir);
    }
  } catch (error) {
    console.error('Error selecting directory:', error);
    if (error && error.message) {
      showStatus('选择目录失败: ' + error.message, 'error');
    } else {
      showStatus('选择目录失败: ' + error, 'error');
    }
  }
}

// 更新分割按钮状态
function updateSplitButtonState() {
  const isValid = csvFilePath && outputDir && rowsPerFileInput.value > 0;
  splitBtn.disabled = !isValid;
}

// 开始CSV分割
async function startCsvSplit() {
  const hasHeader = hasHeaderCheckbox.checked;
  const rowsPerFile = parseInt(rowsPerFileInput.value);
  
  if (rowsPerFile <= 0) {
    showStatus('请输入有效的行数', 'error');
    return;
  }
  
  try {
    // 显示进度条
    showProgress(true);
    updateProgress(0, '开始分割...');
    
    // 调用Rust命令进行分割
    const convertToExcel = convertExcelCheckbox.checked;
    const result = await invoke('split_csv', {
      params: {
        input_path: csvFilePath,
        output_dir: outputDir,
        rows_per_file: rowsPerFile,
        has_header: hasHeader,
        convert_to_excel: convertToExcel
      }
    });
    
    // 处理结果
    if (result.success) {
      updateProgress(100, '分割完成！');
      showStatus(`分割完成！共生成 ${result.file_count} 个文件`, 'success');
      
      // 2秒后隐藏进度条
      setTimeout(() => {
        showProgress(false);
      }, 2000);
    } else {
      throw new Error(result.error || '分割失败，请检查文件格式和权限');
    }
    
  } catch (error) {
    showProgress(false);
    console.error('Split error:', error);
    const errorMessage = error?.message || error?.toString() || '未知错误';
    showStatus('分割失败: ' + errorMessage, 'error');
  }
}

// 显示/隐藏进度条
function showProgress(show) {
  if (show) {
    progressContainer.classList.remove('hidden');
  } else {
    progressContainer.classList.add('hidden');
    progressBar.style.width = '0%';
  }
}

// 更新进度
function updateProgress(percent, text) {
  progressBar.style.width = `${percent}%`;
  progressText.textContent = text;
}

// 显示状态消息
function showStatus(message, type = 'info') {
  statusMessage.textContent = message;
  statusMessage.className = `mt-4 text-center text-sm ${getStatusClass(type)}`;
  
  // 5秒后自动清除成功消息
  if (type === 'success') {
    setTimeout(() => {
      statusMessage.textContent = '';
    }, 5000);
  }
}

// 获取状态样式类
function getStatusClass(type) {
  switch (type) {
    case 'success':
      return 'text-green-600';
    case 'error':
      return 'text-red-600';
    case 'info':
    default:
      return 'text-gray-600';
  }
}
