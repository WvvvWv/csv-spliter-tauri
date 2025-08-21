use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::sync::Arc;

use csv::{ReaderBuilder, Writer, WriterBuilder};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tauri::command;

#[derive(Serialize)]
struct SplitResult {
    success: bool,
    file_count: usize,
    error: Option<String>,
}

#[derive(Deserialize)]
struct SplitParams {
    input_path: String,
    output_dir: String,
    rows_per_file: usize,
    has_header: bool,
}

/// 分割CSV文件的主命令
#[command]
async fn split_csv(params: SplitParams) -> Result<SplitResult, String> {
    // 根据文件大小决定是否使用多线程优化
    let input_path = Path::new(&params.input_path);
    let metadata = match std::fs::metadata(input_path) {
        Ok(meta) => meta,
        Err(e) => return Err(format!("无法获取文件信息: {}", e)),
    };
    
    // 对于大文件(>100MB)使用多线程处理
    let use_multithread = metadata.len() > 100 * 1024 * 1024;
    
    if use_multithread {
        match split_csv_multithread(params).await {
            Ok(file_count) => Ok(SplitResult {
                success: true,
                file_count,
                error: None,
            }),
            Err(e) => Ok(SplitResult {
                success: false,
                file_count: 0,
                error: Some(e.to_string()),
            }),
        }
    } else {
        match split_csv_internal(params).await {
            Ok(file_count) => Ok(SplitResult {
                success: true,
                file_count,
                error: None,
            }),
            Err(e) => Ok(SplitResult {
                success: false,
                file_count: 0,
                error: Some(e.to_string()),
            }),
        }
    }
}

/// 内部CSV分割实现
async fn split_csv_internal(params: SplitParams) -> Result<usize, Box<dyn std::error::Error>> {
    let input_path = Path::new(&params.input_path);
    let output_dir = Path::new(&params.output_dir);
    
    // 验证输入文件存在
    if !input_path.exists() {
        return Err(format!("输入文件不存在: {}", params.input_path).into());
    }
    
    // 验证输入文件是有效的CSV文件
    if !input_path.extension().map_or(false, |ext| ext == "csv") {
        return Err("请选择有效的CSV文件".into());
    }
    
    // 检查文件是否为空
    let metadata = std::fs::metadata(input_path)?;
    if metadata.len() == 0 {
        return Err("CSV文件为空".into());
    }
    
    // 创建输出目录（如果不存在）
    if !output_dir.exists() {
        std::fs::create_dir_all(output_dir)
            .map_err(|e| format!("无法创建输出目录: {}", e))?;
    }
    
    // 检查输出目录是否可写
    if let Err(_) = std::fs::File::create(output_dir.join("test_write.tmp")) {
        return Err("输出目录没有写入权限".into());
    } else {
        let _ = std::fs::remove_file(output_dir.join("test_write.tmp"));
    }
    
    // 打开CSV文件
    let file = File::open(input_path)
        .map_err(|e| format!("无法打开CSV文件: {}", e))?;
    
    let mut reader = ReaderBuilder::new()
        .has_headers(params.has_header)
        .from_reader(BufReader::new(file));
    
    // 验证行数参数
    if params.rows_per_file == 0 {
        return Err("每个文件的行数必须大于0".into());
    }
    
    // 读取标题行（如果有）
    let headers = if params.has_header {
        match reader.headers() {
            Ok(headers) => headers.clone(),
            Err(e) => return Err(format!("读取CSV标题行失败: {}", e).into()),
        }
    } else {
        // 如果没有标题行，生成默认列名
        let record_count = match reader.headers() {
            Ok(headers) => headers.len(),
            Err(e) => return Err(format!("读取CSV列数失败: {}", e).into()),
        };
        
        if record_count == 0 {
            return Err("CSV文件没有有效的列数据".into());
        }
        
        csv::StringRecord::from(
            (0..record_count)
                .map(|i| format!("column_{}", i + 1))
                .collect::<Vec<_>>()
        )
    };
    
    let mut record = csv::StringRecord::new();
    let mut current_file_index = 1;
    let mut current_row_count = 0;
    let mut writer: Option<Writer<BufWriter<File>>> = None;
    let mut total_files = 0;
    
    // 获取基础文件名（不含扩展名）
    let file_stem = input_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    
    let mut record_count = 0;
    while let Ok(has_record) = reader.read_record(&mut record) {
        if !has_record {
            break; // 文件结束
        }
        
        record_count += 1;
        
        // 如果需要创建新文件
        if current_row_count == 0 {
            // 关闭之前的writer
            if let Some(mut w) = writer.take() {
                if let Err(e) = w.flush() {
                    return Err(format!("写入文件失败: {}", e).into());
                }
            }
            
            // 创建新文件
            let output_file = output_dir.join(format!("{}_{}.csv", file_stem, current_file_index));
            let file = File::create(&output_file)
                .map_err(|e| format!("无法创建输出文件 {:?}: {}", output_file, e))?;
            
            writer = Some(WriterBuilder::new()
                .from_writer(BufWriter::new(file)));
            
            // 写入标题行
            if let Some(ref mut w) = writer {
                w.write_record(&headers)
                    .map_err(|e| format!("写入标题行失败: {}", e))?;
            }
            
            total_files += 1;
            current_file_index += 1;
        }
        
        // 写入数据行
        if let Some(ref mut w) = writer {
            w.write_record(&record)
                .map_err(|e| format!("写入数据行失败: {}", e))?;
        }
        
        current_row_count += 1;
        
        // 如果达到每文件行数限制，重置计数器
        if current_row_count >= params.rows_per_file {
            current_row_count = 0;
        }
    }
    
    if record_count == 0 {
        return Err("CSV文件没有数据行".into());
    }
    
    // 确保最后一个文件被正确关闭
    if let Some(mut w) = writer {
        w.flush()?;
    }
    
    Ok(total_files)
}

/// 多线程并发CSV分割实现
async fn split_csv_multithread(params: SplitParams) -> Result<usize, Box<dyn std::error::Error>> {
    let input_path = Path::new(&params.input_path);
    let output_dir = Path::new(&params.output_dir);
    
    // 验证输入文件存在
    if !input_path.exists() {
        return Err(format!("输入文件不存在: {}", params.input_path).into());
    }
    
    // 创建输出目录（如果不存在）
    if !output_dir.exists() {
        std::fs::create_dir_all(output_dir)
            .map_err(|e| format!("无法创建输出目录: {}", e))?;
    }
    
    // 验证行数参数
    if params.rows_per_file == 0 {
        return Err("每个文件的行数必须大于0".into());
    }
    
    // 获取基础文件名
    let file_stem = input_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    
    // 首先读取整个文件并计算总行数
    let file = File::open(input_path)?;
    let mut reader = ReaderBuilder::new()
        .has_headers(params.has_header)
        .from_reader(BufReader::new(file));
    
    // 读取所有记录到内存
    let mut records = Vec::new();
    let mut record = csv::StringRecord::new();
    
    while reader.read_record(&mut record)? {
        records.push(record.clone());
    }
    
    if records.is_empty() {
        return Err("CSV文件没有数据行".into());
    }
    
    // 读取标题行
    let headers = if params.has_header {
        match reader.headers() {
            Ok(headers) => headers.clone(),
            Err(_) => {
                // 如果无法读取标题，生成默认列名
                let record_count = records[0].len();
                csv::StringRecord::from(
                    (0..record_count)
                        .map(|i| format!("column_{}", i + 1))
                        .collect::<Vec<_>>()
                )
            }
        }
    } else {
        let record_count = records[0].len();
        csv::StringRecord::from(
            (0..record_count)
                .map(|i| format!("column_{}", i + 1))
                .collect::<Vec<_>>()
        )
    };
    
    // 计算需要创建的文件数量
    let total_records = records.len();
    let file_count = (total_records + params.rows_per_file - 1) / params.rows_per_file;
    
    if file_count == 0 {
        return Ok(0);
    }
    
    // 使用多线程并行处理每个文件
    let records_arc = Arc::new(records);
    let headers_arc = Arc::new(headers);
    
    // 创建文件索引范围用于并行处理
    let file_indices: Vec<usize> = (1..=file_count).collect();
    
    // 并行处理每个文件
    let results: Result<Vec<()>, String> = file_indices
        .into_par_iter()
        .map(|file_index| -> Result<(), String> {
            let records = Arc::clone(&records_arc);
            let headers = Arc::clone(&headers_arc);
            
            // 计算当前文件的起始和结束索引
            let start_idx = (file_index - 1) * params.rows_per_file;
            let end_idx = std::cmp::min(start_idx + params.rows_per_file, total_records);
            
            if start_idx >= total_records {
                return Ok(());
            }
            
            // 创建输出文件
            let output_file = output_dir.join(format!("{}_{}.csv", file_stem, file_index));
            let file = File::create(&output_file)
                .map_err(|e| format!("无法创建输出文件 {:?}: {}", output_file, e))?;
            let mut writer = WriterBuilder::new().from_writer(BufWriter::new(file));
            
            // 写入标题行
            writer.write_record(&*headers)
                .map_err(|e| format!("写入标题行失败: {}", e))?;
            
            // 写入数据行
            for i in start_idx..end_idx {
                if let Some(record) = records.get(i) {
                    writer.write_record(record)
                        .map_err(|e| format!("写入数据行失败: {}", e))?;
                }
            }
            
            writer.flush()
                .map_err(|e| format!("刷新文件失败: {}", e))?;
            Ok(())
        })
        .collect();
    
    // 检查所有线程的结果
    results?;
    
    Ok(file_count)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![split_csv])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
