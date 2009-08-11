class Calendar < ActiveRecord::Base
  has_many :events
  
  def events_on_day(day)
    events.on_day(day)
  end

  def events_in_month(date)
    month = Date.civil(date.year, date.month)
    events.in_month(month)
  end
  
  
  #this exists because there is a bug in rails 2.3 
  # which causes rendering of json for sub classes via arrays to fail.
  def events_in_json(options={})
    
    events = case
      when options[:date]; events_in_span(options[:date], options[:span])
      when options[:start] && options[:end]; events_between(options[:start], options[:end])
      else events = self.events
    end
      
    "[ %s ]" % events.collect{|e| e.to_public_json }.join(',')
  end
end
