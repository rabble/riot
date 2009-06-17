class Event < ActiveRecord::Base
  
  
  def start_datetime
    return Time.at(start_epoch).utc unless timezone 
    debugger
    puts 'oh mys'
  end
  
  def end_datetime
    return Time.at(end_epoch).utc unless timezone 
    Time.at(end_epoch)
  end
  
end
