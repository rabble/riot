module FormHelper
  def search_tag(options = {})
    tag :input, {:type => :search}.update(options)
  end

  def field_wrap(label, field, options = {})
    class_name = "form"
    class_name << options[:group_class] unless options[:group_class].nil?
    ret = '<dl class="' + class_name + '">'
    ret << '<dt>' + label + '</dt>'
    ret << '<dd>' + field + '</dd>'
    ret << '</dl>'
    ret
  end

  def text_group(text, method, options ={})
    label = @template.label(@object_name, method, text, options.merge(:object => @object))
    field = @template.text_field(@object_name, method, options.merge(:object => @object, :class => "textfield"))
    field_wrap(label, field)
  end

  def password_group(text, method, options ={})
    label = @template.label(@object_name, method, text, options.merge(:object => @object))
    field = @template.password_field(@object_name, method, options.merge(:object => @object, :class => "textfield"))
    field_wrap(label, field)
  end

  def text_area_group(text, method, options ={})
    label = @template.label(@object_name, method, text, options.merge(:object => @object))
    field = @template.text_area(@object_name, method, options.merge(:object => @object))
    field_wrap(label, field)
  end

  def select_group(text, method, choices, options = {}, html_options = {})
    label = @template.label(@object_name, method, text, options.merge(:object => @object))
    field = @template.select(@object_name, method, choices, options.merge(:object => @object), html_options)
    field_wrap(label, field, html_options)
  end

  def check_box_group(text, method, options = {}, checked_value = "1", unchecked_value = "0")
        label = @template.label(@object_name, method, text, options.merge(:object => @object))
        field = @template.check_box(@object_name, method, options.merge(:object => @object, :class => "checkbox"), checked_value, unchecked_value)
        field_wrap(label, field)
  end

  def file_group(text, method, options = {})
    label = @template.label(@object_name, method, text, options.merge(:object => @object))
    field = @template.file_field(@object_name, method, options.merge(:object => @object, :class => "filefield"))
    field_wrap(label, field)
  end

  def js_date_group(text, method, options = {})
    text_id = @object_name.to_s + "_" + method.to_s + '_jsdate'

    label = @template.label(@object_name, method, text, options.merge(:object => @object))
    field = @template.hidden_field(@object_name, method, options.merge(:object => @object))

    field << '<input type="text" class="textfield datepicker" update_field="' + @object_name.to_s + '_' + method.to_s + '" update_text="' + text_id + '" />'
    field << '<p class="jsdate" id="' + text_id + '">N/A</p>'
    field_wrap(label, field)
  end

  def submit_group(submit_text = 'Save', cancel_text = 'Cancel', options = {})
    ret = '<div class="form-actions">'
    ret << '<a href="javascript:history.go(-1)" class="cancel-link">' + cancel_text + '</a>'
    ret << @template.submit_tag(submit_text, options.merge(:class => "submit-button"))
    ret << '</div>'
  end
  
  def file_group(text, method, options = {})
    label = @template.label(@object_name, method, text, options.merge(:object => @object))
    field = @template.file_field(@object_name, method, options.merge(:object => @object, :class => "filefield"))
    field_wrap(label, field)
  end
  
  def js_date_group(text, method, options = {})
    text_id = @object_name.to_s + "_" + method.to_s + '_jsdate'
    
    label = @template.label(@object_name, method, text, options.merge(:object => @object))
    field = @template.hidden_field(@object_name, method, options.merge(:object => @object))
    
    field << '<input type="text" class="textfield datepicker" update_field="' + @object_name.to_s + '_' + method.to_s + '" update_text="' + text_id + '" />'
    field << '<p class="jsdate" id="' + text_id + '">N/A</p>'
    field_wrap(label, field)
  end
  
  def submit_group(submit_text = 'Save', cancel_text = 'Cancel', options = {})
    ret = '<div class="form-actions">'
    cancel_link = options[:cancel_link] || "javascript:history.go(-1)"
    ret << "<a href=\"#{cancel_link}\" class=\"cancel-link\">" + cancel_text + '</a>'
    ret << @template.submit_tag(submit_text, options.merge(:class => "submit-button"))
    ret << '</div>'
  end
end

ActionView::Helpers::FormBuilder.send :include, FormHelper